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
    CreateCustomer, DeleteCustomer, RetrieveCustomer, RetrieveCustomerReturned, UpdateCustomer,
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

    let created_customer_id = customer.id.to_string();
    match supabase
        .assign_profile_stripe_customer(auth_token, user_id, &created_customer_id)
        .await
    {
        Ok(Some(assigned_customer_id)) if assigned_customer_id == created_customer_id => {
            Ok(Some(assigned_customer_id))
        }
        Ok(Some(assigned_customer_id)) => {
            delete_unassigned_customer(stripe, &created_customer_id).await;
            Ok(Some(assigned_customer_id))
        }
        Ok(None) => {
            delete_unassigned_customer(stripe, &created_customer_id).await;
            Ok(None)
        }
        Err(assignment_error) => {
            let profiles: Vec<Profile> = match supabase
                .select(
                    "profiles",
                    auth_token,
                    "stripe_customer_id",
                    &[("id", &format!("eq.{}", user_id))],
                )
                .await
            {
                Ok(profiles) => profiles,
                Err(_) => return Err(assignment_error),
            };
            let assigned_customer_id = profiles
                .first()
                .and_then(|profile| profile.stripe_customer_id.clone());
            if assigned_customer_id.as_deref() != Some(&created_customer_id) {
                delete_unassigned_customer(stripe, &created_customer_id).await;
            }
            match assigned_customer_id {
                Some(customer_id) => Ok(Some(customer_id)),
                None => Err(assignment_error),
            }
        }
    }
}

async fn delete_unassigned_customer(stripe: &stripe::Client, customer_id: &str) {
    match DeleteCustomer::new(customer_id).send(stripe).await {
        Ok(_) | Err(stripe::StripeError::Stripe(_, 404)) => {}
        Err(error) => {
            tracing::error!(
                error = %error,
                hyprnote.billing.customer.id = %customer_id,
                "unassigned_stripe_customer_deletion_failed"
            );
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CustomerOwnership {
    Owned,
    Claimable,
    Unowned,
}

fn customer_ownership(
    metadata: Option<&HashMap<String, String>>,
    customer_email: Option<&str>,
    user_id: &str,
    user_email: Option<&str>,
) -> CustomerOwnership {
    let owner_ids = metadata
        .into_iter()
        .flat_map(|values| {
            ["userId", "user_id", "userID"]
                .into_iter()
                .filter_map(|key| values.get(key))
        })
        .filter(|owner_id| !owner_id.is_empty())
        .collect::<Vec<_>>();

    if !owner_ids.is_empty() {
        return if owner_ids
            .iter()
            .all(|owner_id| owner_id.as_str() == user_id)
        {
            CustomerOwnership::Owned
        } else {
            CustomerOwnership::Unowned
        };
    }

    if customer_email
        .zip(user_email)
        .is_some_and(|(customer_email, user_email)| {
            customer_email
                .trim()
                .eq_ignore_ascii_case(user_email.trim())
        })
    {
        CustomerOwnership::Claimable
    } else {
        CustomerOwnership::Unowned
    }
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

    match customer_ownership(
        customer.metadata.as_ref(),
        customer.email.as_deref(),
        user_id,
        user_email,
    ) {
        CustomerOwnership::Owned => {}
        CustomerOwnership::Claimable => {
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
        CustomerOwnership::Unowned => {
            return Err(SubscriptionError::Stripe(
                "Stripe customer does not belong to authenticated user".to_string(),
            ));
        }
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
    use std::collections::HashMap;

    use serde_json::json;

    use crate::trial::pro_trial_days;

    use super::{
        CustomerOwnership, build_trial_subscription, customer_ownership,
        trial_subscription_idempotency_key,
    };

    #[test]
    fn every_nonempty_customer_owner_alias_must_match() {
        let metadata: HashMap<String, String> = [
            ("userId".to_string(), "owner-user".to_string()),
            ("user_id".to_string(), "other-user".to_string()),
            ("userID".to_string(), "owner-user".to_string()),
        ]
        .into();

        assert_eq!(
            customer_ownership(
                Some(&metadata),
                Some("owner@example.com"),
                "owner-user",
                Some("owner@example.com"),
            ),
            CustomerOwnership::Unowned
        );
    }

    #[test]
    fn email_is_claimable_only_without_nonempty_owner_aliases() {
        let conflicting_metadata: HashMap<String, String> = [
            ("userId".to_string(), String::new()),
            ("user_id".to_string(), "other-user".to_string()),
        ]
        .into();
        let empty_metadata: HashMap<String, String> = [
            ("userId".to_string(), String::new()),
            ("user_id".to_string(), String::new()),
        ]
        .into();

        assert_eq!(
            customer_ownership(
                Some(&conflicting_metadata),
                Some("owner@example.com"),
                "owner-user",
                Some("owner@example.com"),
            ),
            CustomerOwnership::Unowned
        );
        assert_eq!(
            customer_ownership(
                Some(&empty_metadata),
                Some("owner@example.com"),
                "owner-user",
                Some("owner@example.com"),
            ),
            CustomerOwnership::Claimable
        );
    }

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
