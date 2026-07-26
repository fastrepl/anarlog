alter table "stripe"."products"
add column IF NOT EXISTS "default_price" text;
