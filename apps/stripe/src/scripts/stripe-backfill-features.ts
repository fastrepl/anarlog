// Sync features from Stripe's entitlements API to the local database.
// Features define what entitlements can be granted to customers.
// https://docs.stripe.com/api/entitlements/feature/list
import { Effect, Schedule } from "effect";
import pg from "pg";
import Stripe from "stripe";
import { parseArgs } from "util";

import { STRIPE_API_VERSION } from "../integration/stripe";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    "dry-run": {
      type: "boolean",
      default: false,
    },
  },
  strict: true,
  allowPositionals: false,
});

const dryRun = values["dry-run"] ?? false;

const { STRIPE_SECRET_KEY, DATABASE_URL } = Bun.env;

if (!STRIPE_SECRET_KEY || !DATABASE_URL) {
  throw new Error(
    "Missing required STRIPE_SECRET_KEY or DATABASE_URL environment variables",
  );
}

const pool = new pg.Pool({ connectionString: DATABASE_URL });
const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});

class DbError {
  readonly _tag = "DbError";
  constructor(readonly message: string) {}
}

const isRateLimitError = (error: unknown): boolean =>
  error instanceof Stripe.errors.StripeError && error.code === "rate_limit";

const retryPolicy = Schedule.exponential("500 millis").pipe(
  Schedule.jittered,
  Schedule.whileInput(isRateLimitError),
  Schedule.intersect(Schedule.recurs(5)),
);

const fetchFeaturesFromStripe = Effect.tryPromise({
  try: async () => {
    const features: Stripe.Entitlements.Feature[] = [];
    for await (const feature of stripe.entitlements.features.list()) {
      features.push(feature);
    }
    return features;
  },
  catch: (error) => error,
}).pipe(Effect.retry(retryPolicy));

const upsertFeature = (feature: Stripe.Entitlements.Feature) =>
  Effect.tryPromise({
    try: () =>
      pool.query(
        `INSERT INTO stripe.features (id, object, livemode, name, lookup_key, active, metadata, last_synced_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (id) DO UPDATE SET
           object = EXCLUDED.object,
           livemode = EXCLUDED.livemode,
           name = EXCLUDED.name,
           lookup_key = EXCLUDED.lookup_key,
           active = EXCLUDED.active,
           metadata = EXCLUDED.metadata,
           last_synced_at = EXCLUDED.last_synced_at`,
        [
          feature.id,
          feature.object,
          feature.livemode,
          feature.name,
          feature.lookup_key,
          feature.active,
          JSON.stringify(feature.metadata || {}),
          new Date().toISOString(),
        ],
      ),
    catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
  }).pipe(
    Effect.map(() => true),
    Effect.catchAll((e) =>
      Effect.gen(function* () {
        yield* Effect.logError(
          `Failed to upsert feature ${feature.id}: ${e.message}`,
        );
        return false;
      }),
    ),
  );

const program = Effect.gen(function* () {
  yield* Effect.log(
    `Starting Stripe features sync${dryRun ? " (DRY RUN)" : ""}...`,
  );

  const features = yield* fetchFeaturesFromStripe;

  yield* Effect.log(`Found ${features.length} features in Stripe`);

  if (features.length === 0) {
    yield* Effect.log("No features found in Stripe. Nothing to sync.");
    return;
  }

  for (const feature of features) {
    yield* Effect.log(
      `  - ${feature.lookup_key}: ${feature.name} (active: ${feature.active})`,
    );
  }

  if (dryRun) {
    yield* Effect.log("Dry run complete - no changes made");
    return;
  }

  let synced = 0;
  let errors = 0;

  for (const feature of features) {
    const success = yield* upsertFeature(feature);
    if (success) {
      synced++;
    } else {
      errors++;
    }
  }

  yield* Effect.log(`Sync complete: synced=${synced}, errors=${errors}`);
});

Effect.runPromise(program)
  .catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  })
  .finally(() => {
    pool.end();
  });
