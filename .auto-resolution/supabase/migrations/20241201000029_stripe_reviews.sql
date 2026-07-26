create table
    if not exists "stripe"."reviews" (
        "id" text primary key,
        object text,
        billing_zip text,
        charge text,
        created integer,
        closed_reason text,
        livemode boolean,
        ip_address text,
        ip_address_location jsonb,
        open boolean,
        opened_reason text,
        payment_intent text,
        reason text,
        session text,
        updated_at timestamptz default timezone('utc'::text, now()) not null
    );

create index stripe_reviews_charge_idx on "stripe"."reviews" using btree (charge);

create index stripe_reviews_payment_intent_idx on "stripe"."reviews" using btree (payment_intent);

create trigger handle_updated_at
    before update
    on stripe.reviews
    for each row
    execute procedure set_updated_at();
