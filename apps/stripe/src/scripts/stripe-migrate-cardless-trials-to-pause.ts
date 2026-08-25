// Preserve existing cardless trial subscriptions after their trial ends.
//
// Native Anarlog trials historically used missing_payment_method=cancel. A
// customer who paid after the trial therefore received a second subscription,
// so Stripe could not attribute the payment to the original trial. Pausing the
// original subscription keeps it resumable without requiring a card at signup.
//
// https://docs.stripe.com/billing/subscriptions/trials/free-trials
import Stripe from "stripe";
import { parseArgs } from "util";

const STRIPE_API_VERSION = "2026-02-25.clover";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    apply: { type: "boolean", default: false },
    limit: { type: "string" },
    price: { type: "string", multiple: true },
    subscription: { type: "string" },
  },
  strict: true,
  allowPositionals: false,
});

const apply = values.apply ?? false;
const limit = values.limit ? parsePositiveInteger(values.limit, "limit") : null;
const priceIds = new Set(values.price ?? []);
const subscriptionId = values.subscription ?? null;
const { STRIPE_SECRET_KEY } = Bun.env;

if (!STRIPE_SECRET_KEY) {
  throw new Error("Missing required STRIPE_SECRET_KEY environment variable");
}
if (!subscriptionId && priceIds.size === 0) {
  throw new Error(
    "Pass at least one --price price_... or restrict the run with --subscription sub_...",
  );
}

const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});

const matchesPrice = (subscription: Stripe.Subscription) =>
  priceIds.size === 0 ||
  subscription.items.data.some((item) => priceIds.has(item.price.id));

const needsPauseEndBehavior = (subscription: Stripe.Subscription) =>
  subscription.status === "trialing" &&
  subscription.trial_settings?.end_behavior.missing_payment_method ===
    "cancel" &&
  matchesPrice(subscription);

const collectCandidates = async () => {
  if (subscriptionId) {
    const subscription = await stripe.subscriptions.retrieve(subscriptionId);
    return needsPauseEndBehavior(subscription) ? [subscription] : [];
  }

  const candidates: Stripe.Subscription[] = [];
  for await (const subscription of stripe.subscriptions.list({
    status: "trialing",
    limit: 100,
  })) {
    if (needsPauseEndBehavior(subscription)) {
      candidates.push(subscription);
      if (limit !== null && candidates.length >= limit) {
        break;
      }
    }
  }
  return candidates;
};

const candidates = await collectCandidates();
console.log(
  `${apply ? "Applying" : "Dry run"}: ${candidates.length} cardless trial subscription(s) will pause instead of cancel`,
);

let updated = 0;
let errors = 0;

for (let offset = 0; offset < candidates.length; offset += 10) {
  await Promise.all(
    candidates.slice(offset, offset + 10).map(async (subscription) => {
      console.log(`  ${subscription.id}`);
      if (!apply) {
        return;
      }

      try {
        await stripe.subscriptions.update(subscription.id, {
          trial_settings: {
            end_behavior: { missing_payment_method: "pause" },
          },
        });
        updated++;
      } catch (error) {
        errors++;
        console.error(`  ${subscription.id}: ${String(error)}`);
      }
    }),
  );
}

console.log(
  JSON.stringify({
    mode: apply ? "apply" : "dry-run",
    matched: candidates.length,
    updated,
    errors,
  }),
);

if (errors > 0) {
  process.exitCode = 1;
}

function parsePositiveInteger(value: string, name: string) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}
