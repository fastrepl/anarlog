import { StripeSync } from "@supabase/stripe-sync-engine";

export function createStripeSync({
  databaseUrl,
  stripeApiVersion,
  stripeSecretKey,
  stripeWebhookSecret,
}: {
  databaseUrl: string;
  stripeApiVersion: string;
  stripeSecretKey: string;
  stripeWebhookSecret: string;
}) {
  return new StripeSync({
    schema: "stripe",
    poolConfig: { connectionString: databaseUrl },
    stripeSecretKey,
    stripeWebhookSecret,
    stripeApiVersion,
    backfillRelatedEntities: true,
    // Stripe event timestamps have one-second precision, so same-second lifecycle
    // events must reconcile against the current subscription instead of payload order.
    revalidateObjectsViaStripeApi: ["subscription"],
  });
}
