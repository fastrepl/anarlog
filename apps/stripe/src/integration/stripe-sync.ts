import { StripeSync } from "@supabase/stripe-sync-engine";

import { env } from "../env";
import { STRIPE_API_VERSION } from "./stripe";

export const stripeSync = new StripeSync({
  schema: "stripe",
  poolConfig: { connectionString: env.DATABASE_URL },
  stripeSecretKey: env.STRIPE_SECRET_KEY,
  stripeWebhookSecret: env.STRIPE_WEBHOOK_SECRET,
  stripeApiVersion: STRIPE_API_VERSION,
  backfillRelatedEntities: true,
});
