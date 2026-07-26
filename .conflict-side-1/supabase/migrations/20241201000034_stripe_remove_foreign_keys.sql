-- Remove all foreign key constraints

ALTER TABLE "stripe"."subscriptions" DROP CONSTRAINT IF EXISTS "subscriptions_customer_fkey";

ALTER TABLE "stripe"."prices" DROP CONSTRAINT IF EXISTS "prices_product_fkey";

ALTER TABLE "stripe"."invoices" DROP CONSTRAINT IF EXISTS "invoices_customer_fkey";

ALTER TABLE "stripe"."invoices" DROP CONSTRAINT IF EXISTS "invoices_subscription_fkey";

ALTER TABLE "stripe"."subscription_items" DROP CONSTRAINT IF EXISTS "subscription_items_price_fkey";

ALTER TABLE "stripe"."subscription_items" DROP CONSTRAINT IF EXISTS "subscription_items_subscription_fkey";
