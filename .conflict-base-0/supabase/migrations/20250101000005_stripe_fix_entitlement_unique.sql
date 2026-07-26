alter table "stripe"."active_entitlements"
    drop constraint if exists "active_entitlements_lookup_key_key";

alter table "stripe"."active_entitlements"
    add constraint "active_entitlements_customer_lookup_key_key"
    unique ("customer", "lookup_key");
