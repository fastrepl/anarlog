create table if not exists "stripe"."payment_methods" (
    id text primary key,
    object text,
    created integer,
    customer text,
    type text,
    billing_details jsonb,
    metadata jsonb,
    card jsonb
);

CREATE INDEX stripe_payment_methods_customer_idx ON "stripe"."payment_methods" USING btree (customer);