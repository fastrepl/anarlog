import { PostHog } from "posthog-node";
import Stripe from "stripe";
import { parseArgs } from "util";

import { getInvoiceAnalyticsProperties } from "../analytics-payload";
import { getCustomerUserId } from "../customer-metadata";

const STRIPE_API_VERSION = "2026-02-25.clover";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    apply: { type: "boolean", default: false },
  },
  strict: true,
  allowPositionals: false,
});

const apply = values.apply ?? false;
const { POSTHOG_API_KEY, STRIPE_SECRET_KEY } = Bun.env;

if (!POSTHOG_API_KEY || !STRIPE_SECRET_KEY) {
  throw new Error("Missing POSTHOG_API_KEY or STRIPE_SECRET_KEY");
}

const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});
const posthog = apply
  ? new PostHog(POSTHOG_API_KEY, {
      host: "https://us.i.posthog.com",
      flushAt: 100,
      flushInterval: 0,
    })
  : null;

const totals = {
  scanned: 0,
  captured: 0,
  free: 0,
  missingCustomer: 0,
  missingUser: 0,
};

for await (const invoice of stripe.invoices.list({
  status: "paid",
  limit: 100,
  expand: ["data.customer"],
})) {
  totals.scanned++;

  if (invoice.amount_paid <= 0) {
    totals.free++;
    continue;
  }

  const customer = invoice.customer;
  if (
    !customer ||
    typeof customer === "string" ||
    ("deleted" in customer && customer.deleted)
  ) {
    totals.missingCustomer++;
    continue;
  }

  const userId = getCustomerUserId(customer.metadata);
  if (!userId) {
    totals.missingUser++;
    continue;
  }

  totals.captured++;
  posthog?.capture({
    distinctId: userId,
    event: "subscription_payment_succeeded",
    timestamp: new Date(
      (invoice.status_transitions.paid_at ?? invoice.created) * 1000,
    ),
    properties: {
      ...getInvoiceAnalyticsProperties(invoice),
      $insert_id: `stripe-invoice:${invoice.id}`,
      source: "stripe",
      backfilled: true,
    },
  });
}

await posthog?.shutdown();

console.log(
  JSON.stringify({
    mode: apply ? "apply" : "dry-run",
    ...totals,
  }),
);
