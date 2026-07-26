
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'invoice_status') THEN
        create type "stripe"."invoice_status" as enum ('draft', 'open', 'paid', 'uncollectible', 'void');
    END IF;
END
$$;


create table if not exists "stripe"."invoices" (
  "id" text primary key,
  "object" text,
  "auto_advance" boolean,
  "collection_method" text,
  "currency" text,
  "description" text,
  "hosted_invoice_url" text,
  "lines" jsonb,
  "metadata" jsonb,
  "period_end" integer,
  "period_start" integer,
  "status" "stripe"."invoice_status",
  "total" bigint,
  "account_country" text,
  "account_name" text,
  "account_tax_ids" jsonb,
  "amount_due" bigint,
  "amount_paid" bigint,
  "amount_remaining" bigint,
  "application_fee_amount" bigint,
  "attempt_count" integer,
  "attempted" boolean,
  "billing_reason" text,
  "created" integer,
  "custom_fields" jsonb,
  "customer_address" jsonb,
  "customer_email" text,
  "customer_name" text,
  "customer_phone" text,
  "customer_shipping" jsonb,
  "customer_tax_exempt" text,
  "customer_tax_ids" jsonb,
  "default_tax_rates" jsonb,
  "discount" jsonb,
  "discounts" jsonb,
  "due_date" integer,
  "ending_balance" integer,
  "footer" text,
  "invoice_pdf" text,
  "last_finalization_error" jsonb,
  "livemode" boolean,
  "next_payment_attempt" integer,
  "number" text,
  "paid" boolean,
  "payment_settings" jsonb,
  "post_payment_credit_notes_amount" integer,
  "pre_payment_credit_notes_amount" integer,
  "receipt_number" text,
  "starting_balance" integer,
  "statement_descriptor" text,
  "status_transitions" jsonb,
  "subtotal" integer,
  "tax" integer,
  "total_discount_amounts" jsonb,
  "total_tax_amounts" jsonb,
  "transfer_data" jsonb,
  "webhooks_delivered_at" integer,

  "customer" text references "stripe"."customers",
  "subscription" text references "stripe"."subscriptions",
  "payment_intent" text,  -- not yet implemented
  "default_payment_method" text, -- not yet implemented
  "default_source" text, -- not yet implemented
  "on_behalf_of" text, -- not yet implemented
  "charge" text -- not yet implemented
);
