create table if not exists
  "stripe"."credit_notes" (
    "id" text primary key,
    object text,
    amount integer,
    amount_shipping integer,
    created integer,
    currency text,
    customer text,
    customer_balance_transaction text,
    discount_amount integer,
    discount_amounts jsonb,
    invoice text,
    lines jsonb,
    livemode boolean,
    memo text,
    metadata jsonb,
    number text,
    out_of_band_amount integer,
    pdf text,
    reason text,
    refund text,
    shipping_cost jsonb,
    status text,
    subtotal integer,
    subtotal_excluding_tax integer,
    tax_amounts jsonb,
    total integer,
    total_excluding_tax integer,
    type text,
    voided_at text
  );

create index stripe_credit_notes_customer_idx on "stripe"."credit_notes" using btree (customer);

create index stripe_credit_notes_invoice_idx on "stripe"."credit_notes" using btree (invoice);