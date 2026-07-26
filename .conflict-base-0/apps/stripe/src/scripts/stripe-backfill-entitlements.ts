// Backfill entitlements for customers who subscribed before entitlements were properly set up.
// Unlike stripe-sync-entitlements.ts which relies on Stripe's entitlements API,
// this script manually calculates entitlements based on subscription state.
//
// Stripe's entitlements API only updates at subscription creation or billing cycle,
// so past customers need this backfill.
import { Effect, Schedule } from "effect";
import pg from "pg";
import { parseArgs } from "util";

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

const { DATABASE_URL } = Bun.env;

if (!DATABASE_URL) {
  throw new Error("Missing required DATABASE_URL environment variable");
}

const pool = new pg.Pool({ connectionString: DATABASE_URL });

const PRODUCT_ID = "prod_SHWUtH1i2DPvSD";
const ENTITLEMENT_LOOKUP_KEY = "hyprnote_pro";

class DbError {
  readonly _tag = "DbError";
  constructor(readonly message: string) {}
}

const retryPolicy = Schedule.exponential("100 millis").pipe(
  Schedule.jittered,
  Schedule.intersect(Schedule.recurs(3)),
);

const fetchCustomersWithActiveSubscriptions = Effect.tryPromise({
  try: () =>
    pool.query<{ customer: string }>(
      `SELECT DISTINCT s.customer
       FROM stripe.subscriptions s
       JOIN stripe.subscription_items si ON si.subscription = s.id
       JOIN stripe.prices p ON si.price = p.id
       WHERE s.status IN ('active', 'trialing', 'past_due')
         AND p.product = $1
         AND s.customer IS NOT NULL`,
      [PRODUCT_ID],
    ),
  catch: (e) =>
    new DbError(
      `Failed to fetch subscriptions: ${e instanceof Error ? e.message : String(e)}`,
    ),
}).pipe(Effect.map((result) => new Set(result.rows.map((r) => r.customer))));

const fetchCustomersWithEntitlements = Effect.tryPromise({
  try: () =>
    pool.query<{ customer: string }>(
      `SELECT DISTINCT customer FROM stripe.active_entitlements WHERE lookup_key = $1`,
      [ENTITLEMENT_LOOKUP_KEY],
    ),
  catch: (e) =>
    new DbError(
      `Failed to fetch entitlements: ${e instanceof Error ? e.message : String(e)}`,
    ),
}).pipe(Effect.map((result) => new Set(result.rows.map((r) => r.customer))));

const createEntitlement = (customerId: string) =>
  Effect.tryPromise({
    try: () =>
      pool.query(
        `INSERT INTO stripe.active_entitlements (id, object, livemode, feature, customer, lookup_key, last_synced_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (customer, lookup_key) DO UPDATE SET
           last_synced_at = EXCLUDED.last_synced_at`,
        [
          `backfill_${customerId}_${ENTITLEMENT_LOOKUP_KEY}`,
          "entitlements.active_entitlement",
          true,
          null,
          customerId,
          ENTITLEMENT_LOOKUP_KEY,
          new Date().toISOString(),
        ],
      ),
    catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
  }).pipe(
    Effect.retry(retryPolicy),
    Effect.map(() => true),
    Effect.catchAll((e) =>
      Effect.gen(function* () {
        yield* Effect.logError(
          `Failed to create entitlement for ${customerId}: ${e.message}`,
        );
        return false;
      }),
    ),
  );

const deleteEntitlement = (customerId: string) =>
  Effect.tryPromise({
    try: () =>
      pool.query(
        `DELETE FROM stripe.active_entitlements WHERE customer = $1 AND lookup_key = $2`,
        [customerId, ENTITLEMENT_LOOKUP_KEY],
      ),
    catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
  }).pipe(
    Effect.retry(retryPolicy),
    Effect.map(() => true),
    Effect.catchAll((e) =>
      Effect.gen(function* () {
        yield* Effect.logError(
          `Failed to delete entitlement for ${customerId}: ${e.message}`,
        );
        return false;
      }),
    ),
  );

const program = Effect.gen(function* () {
  yield* Effect.log(
    `Starting entitlement backfill${dryRun ? " (DRY RUN)" : ""}...`,
  );
  yield* Effect.log(`Product ID: ${PRODUCT_ID}`);
  yield* Effect.log(`Entitlement lookup_key: ${ENTITLEMENT_LOOKUP_KEY}`);

  const [activeCustomers, entitledCustomers] = yield* Effect.all([
    fetchCustomersWithActiveSubscriptions,
    fetchCustomersWithEntitlements,
  ]);

  yield* Effect.log(
    `Found ${activeCustomers.size} customers with active subscriptions`,
  );
  yield* Effect.log(
    `Found ${entitledCustomers.size} customers with existing entitlements`,
  );

  const toAdd = [...activeCustomers].filter((c) => !entitledCustomers.has(c));
  const toRemove = [...entitledCustomers].filter(
    (c) => !activeCustomers.has(c),
  );

  yield* Effect.log(`Customers needing entitlements added: ${toAdd.length}`);
  yield* Effect.log(
    `Customers needing entitlements removed: ${toRemove.length}`,
  );

  if (dryRun) {
    if (toAdd.length > 0) {
      yield* Effect.log(`Would add entitlements for: ${toAdd.join(", ")}`);
    }
    if (toRemove.length > 0) {
      yield* Effect.log(
        `Would remove entitlements for: ${toRemove.join(", ")}`,
      );
    }
    yield* Effect.log("Dry run complete - no changes made");
    return;
  }

  let added = 0;
  let removed = 0;
  let errors = 0;

  for (const customerId of toAdd) {
    const success = yield* createEntitlement(customerId);
    if (success) {
      added++;
    } else {
      errors++;
    }
  }

  for (const customerId of toRemove) {
    const success = yield* deleteEntitlement(customerId);
    if (success) {
      removed++;
    } else {
      errors++;
    }
  }

  yield* Effect.log(
    `Backfill complete: added=${added}, removed=${removed}, errors=${errors}`,
  );
});

Effect.runPromise(program)
  .catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  })
  .finally(() => {
    pool.end();
  });
