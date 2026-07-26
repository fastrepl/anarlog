create table
    if not exists "stripe"."active_entitlements" (
        "id" text primary key,
        "object" text,
        "livemode" boolean,
        "feature" text,
        "customer" text,
        "lookup_key" text unique,
        "updated_at" timestamptz default timezone('utc'::text, now()) not null,
        "last_synced_at" timestamptz
    );

create index stripe_active_entitlements_customer_idx on "stripe"."active_entitlements" using btree (customer);
create index stripe_active_entitlements_feature_idx on "stripe"."active_entitlements" using btree (feature);

create trigger handle_updated_at
    before update
    on stripe.active_entitlements
    for each row
    execute procedure set_updated_at();
