create table if not exists "stripe"."subscription_items" (
  "id" text primary key,
  "object" text,
  "billing_thresholds" jsonb,
  "created" integer,
  "deleted" boolean,
  "metadata" jsonb,
  "quantity" integer,
  "price" text references "stripe"."prices",
  "subscription" text references "stripe"."subscriptions",
  "tax_rates" jsonb
);