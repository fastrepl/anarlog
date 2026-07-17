use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;
use stripe::StripeRequest;
use stripe_billing::subscription::{
    CreateSubscription, CreateSubscriptionItems, CreateSubscriptionTrialSettings,
    CreateSubscriptionTrialSettingsEndBehavior,
    CreateSubscriptionTrialSettingsEndBehaviorMissingPaymentMethod,
};
use stripe_core::customer::{
    CreateCustomer, DeleteCustomer, RetrieveCustomer, RetrieveCustomerReturned,
};

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
        let email = supabase.get_user_email(auth_token).await?;
        if stripe_customer_belongs_to_user(stripe, customer_id, user_id, email.as_deref()).await? {
            return Ok(Some(customer_id.clone()));
        }
        return Err(SubscriptionError::Internal(
            "Stripe customer ownership could not be verified".to_string(),
        ));
    }

    let email = supabase.get_user_email(auth_token).await?;

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

async fn stripe_customer_belongs_to_user(
    stripe: &stripe::Client,
    customer_id: &str,
    user_id: &str,
    user_email: Option<&str>,
) -> Result<bool> {
    let customer = match RetrieveCustomer::new(customer_id).send(stripe).await {
        Ok(RetrieveCustomerReturned::Customer(customer)) => customer,
        Ok(RetrieveCustomerReturned::DeletedCustomer(_))
        | Err(stripe::StripeError::Stripe(_, 404)) => return Ok(false),
        Err(error) => return Err(SubscriptionError::Stripe(error.to_string())),
    };
    let metadata_user_ids = customer
        .metadata
        .as_ref()
        .map(|metadata| {
            ["userId", "user_id", "userID"]
                .into_iter()
                .filter_map(|key| metadata.get(key).map(String::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !metadata_user_ids.is_empty() {
        return Ok(metadata_user_ids
            .iter()
            .all(|metadata_user_id| *metadata_user_id == user_id));
    }

    Ok(customer
        .email
        .as_deref()
        .zip(user_email)
        .is_some_and(|(customer_email, user_email)| {
            customer_email.eq_ignore_ascii_case(user_email)
        }))
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
