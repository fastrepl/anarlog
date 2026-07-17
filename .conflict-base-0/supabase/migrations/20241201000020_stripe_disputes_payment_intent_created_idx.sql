ALTER TABLE "stripe"."disputes" ADD COLUMN IF NOT EXISTS payment_intent TEXT;

CREATE INDEX IF NOT EXISTS stripe_dispute_created_idx ON "stripe"."disputes" USING btree (created);