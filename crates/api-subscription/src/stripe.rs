use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use stripe::StripeRequest;
use stripe_billing::subscription::{
    CreateSubscription, CreateSubscriptionItems, CreateSubscriptionTrialSettings,
    CreateSubscriptionTrialSettingsEndBehavior,
    CreateSubscriptionTrialSettingsEndBehaviorMissingPaymentMethod,
};
use stripe_core::customer::CreateCustomer;

use crate::error::{Result, SubscriptionError};
use crate::supabase::SupabaseClient;

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
        return Ok(Some(customer_id.clone()));
    }

    let email = supabase.get_user_email(auth_token).await?;

    let metadata: HashMap<String, String> = [("userId".to_string(), user_id.to_string())].into();

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

    #[derive(Serialize)]
    struct UpdateData {
        stripe_customer_id: String,
    }

    supabase
        .update(
            "profiles",
            auth_token,
            &[
                ("id", &format!("eq.{}", user_id)),
                ("stripe_customer_id", "is.null"),
            ],
            &UpdateData {
                stripe_customer_id: customer.id.to_string(),
            },
        )
        .await?;

    let updated_profiles: Vec<Profile> = supabase
        .select(
            "profiles",
            auth_token,
            "stripe_customer_id",
            &[("id", &format!("eq.{}", user_id))],
        )
        .await?;

    Ok(updated_profiles
        .first()
        .and_then(|p| p.stripe_customer_id.clone()))
}

pub(crate) async fn create_trial_subscription(
    stripe: &stripe::Client,
    customer_id: &str,
    price_id: &str,
    user_id: &str,
) -> Result<()> {
    let mut item = CreateSubscriptionItems::new();
    item.price = Some(price_id.to_string());

    let create_sub = CreateSubscription::new()
        .customer(customer_id)
        .items(vec![item])
        .trial_period_days(14u32)
        .trial_settings(CreateSubscriptionTrialSettings::new(
            CreateSubscriptionTrialSettingsEndBehavior::new(
                CreateSubscriptionTrialSettingsEndBehaviorMissingPaymentMethod::Cancel,
            ),
        ));

    let date = Utc::now().format("%Y-%m-%d").to_string();
    let idempotency_key: stripe::IdempotencyKey = format!("trial-{}-{}", user_id, date)
        .try_into()
        .map_err(|e: stripe::IdempotentKeyError| SubscriptionError::Internal(e.to_string()))?;

    let start = Instant::now();
    create_sub
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

    Ok(())
}
