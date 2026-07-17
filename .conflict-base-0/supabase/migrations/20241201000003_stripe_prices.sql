DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'pricing_type') THEN
        create type "stripe"."pricing_type" as enum ('one_time', 'recurring');
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'pricing_tiers') THEN
      create type "stripe"."pricing_tiers" as enum ('graduated', 'volume');
    END IF;
    --more types here...
END
$$;


create table if not exists "stripe"."prices" (
  "id" text primary key,
  "object" text,
  "active" boolean,
  "currency" text,
  "metadata" jsonb,
  "nickname" text,
  "recurring" jsonb,
  "type" stripe.pricing_type,
  "unit_amount" integer,
  "billing_scheme" text,
  "created" integer,
  "livemode" boolean,
  "lookup_key" text,
  "tiers_mode" stripe.pricing_tiers,
  "transform_quantity" jsonb,
  "unit_amount_decimal" text,

  "product" text references stripe.products
);

