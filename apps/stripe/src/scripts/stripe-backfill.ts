import { StripeSync } from "@supabase/stripe-sync-engine";
import { parseArgs } from "util";

import { STRIPE_API_VERSION } from "../integration/stripe";

const { DATABASE_URL, STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET } = Bun.env;

if (!DATABASE_URL || !STRIPE_SECRET_KEY || !STRIPE_WEBHOOK_SECRET) {
  throw new Error("Missing required environment variables");
}

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    "created-gte": {
      type: "string",
    },
    "created-lte": {
      type: "string",
    },
    help: {
      type: "boolean",
      short: "h",
    },
  },
  strict: true,
  allowPositionals: false,
});

if (values.help) {
  console.log(`
Usage: bun stripe-backfill.ts [options]

Options:
  --created-gte <timestamp>  Only sync objects created at or after this unix timestamp
  --created-lte <timestamp>  Only sync objects created at or before this unix timestamp
  -h, --help                 Show this help message

Examples:
  bun stripe-backfill.ts
  bun stripe-backfill.ts --created-gte 1704067200
  bun stripe-backfill.ts --created-gte 1704067200 --created-lte 1706745600
`);
  process.exit(0);
}

const createdGte = values["created-gte"]
  ? parseInt(values["created-gte"], 10)
  : undefined;
const createdLte = values["created-lte"]
  ? parseInt(values["created-lte"], 10)
  : undefined;

if (values["created-gte"] && isNaN(createdGte!)) {
  throw new Error(
    "Invalid --created-gte value: must be a valid unix timestamp",
  );
}

if (values["created-lte"] && isNaN(createdLte!)) {
  throw new Error(
    "Invalid --created-lte value: must be a valid unix timestamp",
  );
}

const created: { gte?: number; lte?: number } | undefined =
  createdGte !== undefined || createdLte !== undefined
    ? {
        ...(createdGte !== undefined && { gte: createdGte }),
        ...(createdLte !== undefined && { lte: createdLte }),
      }
    : undefined;

const sync = new StripeSync({
  poolConfig: {
    connectionString: DATABASE_URL,
    max: 10,
  },
  schema: "stripe",
  stripeSecretKey: STRIPE_SECRET_KEY,
  stripeWebhookSecret: STRIPE_WEBHOOK_SECRET,
  autoExpandLists: true,
  stripeApiVersion: STRIPE_API_VERSION,
  backfillRelatedEntities: true,
});

if (created) {
  console.log(
    `Starting Stripe backfill with date filter: ${JSON.stringify(created)}`,
  );
} else {
  console.log("Starting Stripe backfill (all objects)...");
}

await sync.syncBackfill({ object: "all", created });
console.log("Backfill complete.");
