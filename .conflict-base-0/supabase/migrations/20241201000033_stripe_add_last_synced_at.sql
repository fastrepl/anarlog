-- Add last_synced_at column to all Stripe tables for tracking sync status

-- Charges
alter table "stripe"."charges"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Coupons
alter table "stripe"."coupons"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Credit Notes
alter table "stripe"."credit_notes"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Customers
alter table "stripe"."customers"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Disputes
alter table "stripe"."disputes"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Early Fraud Warnings
alter table "stripe"."early_fraud_warnings"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Events
alter table "stripe"."events"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Invoices
alter table "stripe"."invoices"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Payment Intents
alter table "stripe"."payment_intents"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Payment Methods
alter table "stripe"."payment_methods"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Payouts
alter table "stripe"."payouts"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Plans
alter table "stripe"."plans"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Prices
alter table "stripe"."prices"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Products
alter table "stripe"."products"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Refunds
alter table "stripe"."refunds"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Reviews
alter table "stripe"."reviews"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Setup Intents
alter table "stripe"."setup_intents"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Subscription Items
alter table "stripe"."subscription_items"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Subscription Schedules
alter table "stripe"."subscription_schedules"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Subscriptions
alter table "stripe"."subscriptions"
add column IF NOT EXISTS "last_synced_at" timestamptz;

-- Tax IDs
alter table "stripe"."tax_ids"
add column IF NOT EXISTS "last_synced_at" timestamptz;
