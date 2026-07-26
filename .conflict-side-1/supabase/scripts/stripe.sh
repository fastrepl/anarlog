#!/bin/bash

set -e

MIGRATIONS_DIR="$(dirname "$0")/../migrations"
BASE_URL="https://raw.githubusercontent.com/supabase/stripe-sync-engine/refs/heads/main/packages/sync-engine/src/database/migrations"

declare -a MIGRATIONS=(
  "0000_initial_migration.sql:20241201000000_stripe_initial_migration.sql"
  "0001_products.sql:20241201000001_stripe_products.sql"
  "0002_customers.sql:20241201000002_stripe_customers.sql"
  "0003_prices.sql:20241201000003_stripe_prices.sql"
  "0004_subscriptions.sql:20241201000004_stripe_subscriptions.sql"
  "0005_invoices.sql:20241201000005_stripe_invoices.sql"
  "0006_charges.sql:20241201000006_stripe_charges.sql"
  "0007_coupons.sql:20241201000007_stripe_coupons.sql"
  "0008_disputes.sql:20241201000008_stripe_disputes.sql"
  "0009_events.sql:20241201000009_stripe_events.sql"
  "0010_payouts.sql:20241201000010_stripe_payouts.sql"
  "0011_plans.sql:20241201000011_stripe_plans.sql"
  "0012_add_updated_at.sql:20241201000012_stripe_add_updated_at.sql"
  "0013_add_subscription_items.sql:20241201000013_stripe_add_subscription_items.sql"
  "0014_migrate_subscription_items.sql:20241201000014_stripe_migrate_subscription_items.sql"
  "0015_add_customer_deleted.sql:20241201000015_stripe_add_customer_deleted.sql"
  "0016_add_invoice_indexes.sql:20241201000016_stripe_add_invoice_indexes.sql"
  "0017_drop_charges_unavailable_columns.sql:20241201000017_stripe_drop_charges_unavailable_columns.sql"
  "0018_setup_intents.sql:20241201000018_stripe_setup_intents.sql"
  "0019_payment_methods.sql:20241201000019_stripe_payment_methods.sql"
  "0020_disputes_payment_intent_created_idx.sql:20241201000020_stripe_disputes_payment_intent_created_idx.sql"
  "0021_payment_intent.sql:20241201000021_stripe_payment_intent.sql"
  "0022_adjust_plans.sql:20241201000022_stripe_adjust_plans.sql"
  "0023_invoice_deleted.sql:20241201000023_stripe_invoice_deleted.sql"
  "0024_subscription_schedules.sql:20241201000024_stripe_subscription_schedules.sql"
  "0025_tax_ids.sql:20241201000025_stripe_tax_ids.sql"
  "0026_credit_notes.sql:20241201000026_stripe_credit_notes.sql"
  "0027_add_marketing_features_to_products.sql:20241201000027_stripe_add_marketing_features_to_products.sql"
  "0028_early_fraud_warning.sql:20241201000028_stripe_early_fraud_warning.sql"
  "0029_reviews.sql:20241201000029_stripe_reviews.sql"
  "0030_refunds.sql:20241201000030_stripe_refunds.sql"
  "0031_add_default_price.sql:20241201000031_stripe_add_default_price.sql"
  "0032_update_subscription_items.sql:20241201000032_stripe_update_subscription_items.sql"
  "0033_add_last_synced_at.sql:20241201000033_stripe_add_last_synced_at.sql"
  "0034_remove_foreign_keys.sql:20241201000034_stripe_remove_foreign_keys.sql"
  "0035_checkout_sessions.sql:20241201000035_stripe_checkout_sessions.sql"
  "0036_checkout_session_line_items.sql:20241201000036_stripe_checkout_session_line_items.sql"
  "0037_add_features.sql:20241201000037_stripe_add_features.sql"
  "0038_active_entitlement.sql:20241201000038_stripe_active_entitlement.sql"
  "0039_add_paused_to_subscription_status.sql:20241201000039_stripe_add_paused_to_subscription_status.sql"
)

echo "Fetching stripe-sync-engine migrations..."

for entry in "${MIGRATIONS[@]}"; do
  SOURCE="${entry%%:*}"
  TARGET="${entry##*:}"
  
  echo "  $SOURCE -> $TARGET"
  curl -sL "$BASE_URL/$SOURCE" -o "$MIGRATIONS_DIR/$TARGET"
done

echo 'create schema if not exists "stripe";' > "$MIGRATIONS_DIR/20241201000000_stripe_initial_migration.sql"
echo "Done. Fetched ${#MIGRATIONS[@]} migrations."
