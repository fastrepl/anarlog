use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;
use stripe::StripeRequest;
use stripe_billing::subscription::{
    CreateSubscription, CreateSubscriptionItems, CreateSubscriptionTrialSettings,
    CreateSubscriptionTrialSettingsEndBehavior,
    CreateSubscriptionTrialSettingsEndBehaviorMissingPaymentMethod, ListSubscription,
    ListSubscriptionStatus,
};
use stripe_core::customer::{
    CreateCustomer, RetrieveCustomer, RetrieveCustomerReturned, UpdateCustomer,
};

use crate::error::{Result, SubscriptionError};
use crate::supabase::SupabaseClient;
use crate::trial::pro_trial_days;

#[derive(Debug, Deserialize)]
struct Profile {
    stripe_customer_id: Option<String>,
}

pub(crate) async fn get_or_create_customer(
    supabase: &SupabaseClient,
    stripe: &stripe::Client,
    auth_token: &str,
    user_id: &str,
) -> Result<Option<String>> {
    let email = supabase.get_user_email(auth_token).await?;
    let profiles: Vec<Profile> = supabase
        .select(
            "profiles",
            auth_token,
            "stripe_customer_id",
            &[("id", &format!("eq.{}", user_id))],
        )
        .await?;

    if let Some(profile) = profiles.first()
        && let Some(customer_id) = &profile.stripe_customer_id
    {
        verify_customer_ownership(stripe, customer_id, user_id, email.as_deref()).await?;
        return Ok(Some(customer_id.clone()));
    }

    let metadata: HashMap<String, String> = [
        ("userId".to_string(), user_id.to_string()),
        (
            "posthog_person_distinct_id".to_string(),
            user_id.to_string(),
        ),
    ]
    .into();

    let mut create_customer = CreateCustomer::new().metadata(metadata);

    if let Some(ref email_str) = email {
        create_customer = create_customer.email(email_str);
    }

    let idempotency_key: stripe::IdempotencyKey = format!("create-customer-{}", user_id)
        .try_into()
        .map_err(|e: stripe::IdempotentKeyError| SubscriptionError::Internal(e.to_string()))?;
    let start = Instant::now();
    let customer = create_customer
        .customize()
        .request_strategy(stripe::RequestStrategy::Idempotent(idempotency_key))
        .send(stripe)
        .await
        .map_err(|e: stripe::StripeError| SubscriptionError::Stripe(e.to_string()))?;
    tracing::info!(
        service.peer.name = "stripe",
        hyprnote.stripe.operation = "create_customer",
        hyprnote.duration_ms = start.elapsed().as_millis() as u64,
        "stripe_request_finished"
    );

    supabase
        .admin_link_stripe_customer(user_id, customer.id.as_str())
        .await
        .map(Some)
}

async fn verify_customer_ownership(
    stripe: &stripe::Client,
    customer_id: &str,
    user_id: &str,
    user_email: Option<&str>,
) -> Result<()> {
    let customer = RetrieveCustomer::new(customer_id)
        .send(stripe)
        .await
        .map_err(|e: stripe::StripeError| SubscriptionError::Stripe(e.to_string()))?;
    let RetrieveCustomerReturned::Customer(customer) = customer else {
        return Err(SubscriptionError::Stripe(
            "Stripe customer is unavailable".to_string(),
        ));
    };

    let metadata = customer.metadata.as_ref();
    let owner_id = metadata.and_then(|values| {
        values
            .get("userId")
            .or_else(|| values.get("user_id"))
            .or_else(|| values.get("userID"))
    });
    let email_matches =
        customer
            .email
            .as_deref()
            .zip(user_email)
            .is_some_and(|(customer_email, user_email)| {
                customer_email
                    .trim()
                    .eq_ignore_ascii_case(user_email.trim())
            });
    let belongs_to_user = owner_id.map_or(email_matches, |owner_id| owner_id == user_id);

    if !belongs_to_user {
        return Err(SubscriptionError::Stripe(
            "Stripe customer does not belong to authenticated user".to_string(),
        ));
    }

    if owner_id.is_none() {
        let metadata: HashMap<String, String> = [
            ("userId".to_string(), user_id.to_string()),
            (
                "posthog_person_distinct_id".to_string(),
                user_id.to_string(),
            ),
        ]
        .into();
        UpdateCustomer::new(customer_id)
            .metadata(metadata)
            .send(stripe)
            .await
            .map_err(|e: stripe::StripeError| SubscriptionError::Stripe(e.to_string()))?;
    }

    Ok(())
}

pub(crate) async fn create_trial_subscription(
    stripe: &stripe::Client,
    customer_id: &str,
    price_id: &str,
    idempotency_key: stripe::IdempotencyKey,
) -> Result<Option<i64>> {
    let create_sub = build_trial_subscription(customer_id, price_id);

    let start = Instant::now();
    let subscription = create_sub
        .customize()
        .request_strategy(stripe::RequestStrategy::Idempotent(idempotency_key))
        .send(stripe)
        .await
        .map_err(|e: stripe::StripeError| SubscriptionError::Stripe(e.to_string()))?;
    tracing::info!(
        service.peer.name = "stripe",
        hyprnote.stripe.operation = "create_trial_subscription",
        hyprnote.duration_ms = start.elapsed().as_millis() as u64,
        "stripe_request_finished"
    );

    Ok(subscription.trial_end)
}

pub(crate) fn trial_subscription_idempotency_key(
    reservation_id: &str,
) -> Result<stripe::IdempotencyKey> {
    format!("trial-{reservation_id}")
        .try_into()
        .map_err(|e: stripe::IdempotentKeyError| SubscriptionError::Internal(e.to_string()))
}

pub(crate) async fn customer_has_subscription_history(
    stripe: &stripe::Client,
    customer_id: &str,
) -> Result<bool> {
    let subscriptions = ListSubscription::new()
        .customer(customer_id)
        .status(ListSubscriptionStatus::All)
        .limit(1)
        .send(stripe)
        .await
        .map_err(|e: stripe::StripeError| SubscriptionError::Stripe(e.to_string()))?;

    Ok(!subscriptions.data.is_empty())
}

fn build_trial_subscription(customer_id: &str, price_id: &str) -> CreateSubscription {
    let mut item = CreateSubscriptionItems::new();
    item.price = Some(price_id.to_string());

    CreateSubscription::new()
        .customer(customer_id)
        .items(vec![item])
        .trial_period_days(pro_trial_days())
        .trial_settings(CreateSubscriptionTrialSettings::new(
            CreateSubscriptionTrialSettingsEndBehavior::new(
                CreateSubscriptionTrialSettingsEndBehaviorMissingPaymentMethod::Cancel,
            ),
        ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::trial::pro_trial_days;

    use super::{build_trial_subscription, trial_subscription_idempotency_key};

    #[test]
    fn native_trials_are_cardless_and_cancel_if_no_card_is_added() {
        let request = serde_json::to_value(build_trial_subscription("cus_test", "price_test"))
            .expect("trial subscription should serialize");
        let request = &request["inner"];

        assert_eq!(request["customer"], json!("cus_test"));
        assert_eq!(request["trial_period_days"], json!(pro_trial_days()));
        assert_eq!(
            request["trial_settings"]["end_behavior"]["missing_payment_method"],
            json!("cancel")
        );
        assert!(request.get("default_payment_method").is_none());
    }

    #[test]
    fn retrying_a_trial_reservation_reuses_the_idempotency_key() {
        let first = trial_subscription_idempotency_key("reservation-123").unwrap();
        let retry = trial_subscription_idempotency_key("reservation-123").unwrap();

        assert_eq!(first, retry);
        assert_eq!(first.as_str(), "trial-reservation-123");
    }
}
