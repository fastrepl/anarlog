CREATE INDEX stripe_invoices_customer_idx ON "stripe"."invoices" USING btree (customer);
CREATE INDEX stripe_invoices_subscription_idx ON "stripe"."invoices" USING btree (subscription);