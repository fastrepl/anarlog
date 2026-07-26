WITH subscriptions AS (
  select jsonb_array_elements(items->'data') as obj from "stripe"."subscriptions"
)
insert into "stripe"."subscription_items"
select obj->>'id' as "id",
  obj->>'object' as "object", 
  obj->'billing_thresholds' as "billing_thresholds", 
  (obj->>'created')::INTEGER as "created", 
  (obj->>'deleted')::BOOLEAN as "deleted", 
  obj->'metadata' as "metadata", 
  (obj->>'quantity')::INTEGER as "quantity", 
  (obj->'price'->>'id')::TEXT as "price", 
  obj->>'subscription' as "subscription", 
  obj->'tax_rates' as "tax_rates"
from subscriptions
on conflict ("id") 
do update set "id" = excluded."id",
  "object" = excluded."object",
  "billing_thresholds" = excluded."billing_thresholds",
  "created" = excluded."created",
  "deleted" = excluded."deleted",
  "metadata" = excluded."metadata",
  "quantity" = excluded."quantity",
  "price" = excluded."price",
  "subscription" = excluded."subscription",
  "tax_rates" = excluded."tax_rates"