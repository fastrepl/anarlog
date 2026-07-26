create table if not exists
  "stripe"."tax_ids" (
    "id" text primary key,
    "object" text,
    "country" text,
    "customer" text,
    "type" text,
    "value" text,
    "created" integer not null,
    "livemode" boolean,
    "owner" jsonb
  );

create index stripe_tax_ids_customer_idx on "stripe"."tax_ids" using btree (customer);