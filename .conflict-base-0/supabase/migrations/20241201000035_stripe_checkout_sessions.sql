create table
    if not exists "stripe"."checkout_sessions" (
        "id" text primary key,
        "object" text,
        "adaptive_pricing" jsonb,
        "after_expiration" jsonb,
        "allow_promotion_codes" boolean,
        "amount_subtotal" integer,
        "amount_total" integer,
        "automatic_tax" jsonb,
        "billing_address_collection" text,
        "cancel_url" text,
        "client_reference_id" text,
        "client_secret" text,
        "collected_information" jsonb,
        "consent" jsonb,
        "consent_collection" jsonb,
        "created" integer,
        "currency" text,
        "currency_conversion" jsonb,
        "custom_fields" jsonb,
        "custom_text" jsonb,
        "customer" text,
        "customer_creation" text,
        "customer_details" jsonb,
        "customer_email" text,
        "discounts" jsonb,
        "expires_at" integer,
        "invoice" text,
        "invoice_creation" jsonb,
        "livemode" boolean,
        "locale" text,
        "metadata" jsonb,
        "mode" text,
        "optional_items" jsonb,
        "payment_intent" text,
        "payment_link" text,
        "payment_method_collection" text,
        "payment_method_configuration_details" jsonb,
        "payment_method_options" jsonb,
        "payment_method_types" jsonb,
        "payment_status" text,
        "permissions" jsonb,
        "phone_number_collection" jsonb,
        "presentment_details" jsonb,
        "recovered_from" text,
        "redirect_on_completion" text,
        "return_url" text,
        "saved_payment_method_options" jsonb,
        "setup_intent" text,
        "shipping_address_collection" jsonb,
        "shipping_cost" jsonb,
        "shipping_details" jsonb,
        "shipping_options" jsonb,
        "status" text,
        "submit_type" text,
        "subscription" text,
        "success_url" text,
        "tax_id_collection" jsonb,
        "total_details" jsonb,
        "ui_mode" text,
        "url" text,
        "wallet_options" jsonb,
        "updated_at" timestamptz default timezone('utc'::text, now()) not null,
        "last_synced_at" timestamptz
    );

create index stripe_checkout_sessions_customer_idx on "stripe"."checkout_sessions" using btree (customer);
create index stripe_checkout_sessions_subscription_idx on "stripe"."checkout_sessions" using btree (subscription);
create index stripe_checkout_sessions_payment_intent_idx on "stripe"."checkout_sessions" using btree (payment_intent);
create index stripe_checkout_sessions_invoice_idx on "stripe"."checkout_sessions" using btree (invoice);

create trigger handle_updated_at
    before update
    on stripe.checkout_sessions
    for each row
    execute procedure set_updated_at();
