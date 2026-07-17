
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'subscription_status') THEN
        create type "stripe"."subscription_status" as enum (
          'trialing',
          'active',
          'canceled',
          'incomplete',
          'incomplete_expired',
          'past_due',
          'unpaid'
        );
    END IF;
END
$$;

create table if not exists "stripe"."subscriptions" (
  "id" text primary key,
  "object" text,
  "cancel_at_period_end" boolean,
  "current_period_end" integer,
  "current_period_start" integer,
  "default_payment_method" text,
  "items" jsonb,
  "metadata" jsonb,
  "pending_setup_intent" text,
  "pending_update" jsonb,
  "status" "stripe"."subscription_status", 
  "application_fee_percent" double precision,
  "billing_cycle_anchor" integer,
  "billing_thresholds" jsonb,
  "cancel_at" integer,
  "canceled_at" integer,
  "collection_method" text,
  "created" integer,
  "days_until_due" integer,
  "default_source" text,
  "default_tax_rates" jsonb,
  "discount" jsonb,
  "ended_at" integer,
  "livemode" boolean,
  "next_pending_invoice_item_invoice" integer,
  "pause_collection" jsonb,
  "pending_invoice_item_interval" jsonb,
  "start_date" integer,
  "transfer_data" jsonb,
  "trial_end" jsonb,
  "trial_start" jsonb,

  "schedule" text,
  "customer" text references "stripe"."customers",
  "latest_invoice" text, -- not yet joined
  "plan" text -- not yet joined
);

