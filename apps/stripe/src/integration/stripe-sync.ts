import { env } from "../env";
import { createStripeSync } from "./create-stripe-sync";
import { STRIPE_API_VERSION } from "./stripe";

export const stripeSync = createStripeSync({
  databaseUrl: env.DATABASE_URL,
  stripeSecretKey: env.STRIPE_SECRET_KEY,
  stripeWebhookSecret: env.STRIPE_WEBHOOK_SECRET,
  stripeApiVersion: STRIPE_API_VERSION,
});
