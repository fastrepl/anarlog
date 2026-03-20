// https://github.com/supabase/stripe-sync-engine/blob/main/packages/sync-engine/README.md#syncing-a-single-entity
// Entitlements can not be synced with "stripe-sync-engine". So we need this script.
//
// Syncs entitlements for customers that are "worth looking at":
// 1. Customers with active/trialing/past_due subscriptions (should have entitlements)
// 2. Customers with existing entitlements (might need updates or cleanup)
//
// This handles both backfill (pre-webhook customers) and daily verification.
import { Effect, Schedule } from "effect";
import pg from "pg";
import Stripe from "stripe";
import { parseArgs } from "util";

import { STRIPE_API_VERSION } from "../integration/stripe";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    "skip-recent-hours": {
      type: "string",
      default: "6",
    },
  },
  strict: true,
  allowPositionals: false,
});

const skipRecentHours = parseInt(values["skip-recent-hours"] ?? "6", 10);

const { STRIPE_SECRET_KEY, DATABASE_URL } = Bun.env;

if (!STRIPE_SECRET_KEY || !DATABASE_URL) {
  throw new Error("Missing required environment variables");
}

const pool = new pg.Pool({ connectionString: DATABASE_URL });
const stripe = new Stripe(STRIPE_SECRET_KEY, {
  apiVersion: STRIPE_API_VERSION,
});

const isRateLimitError = (error: unknown): boolean =>
  error instanceof Stripe.errors.StripeError && error.code === "rate_limit";

const retryPolicy = Schedule.exponential("500 millis").pipe(
  Schedule.jittered,
  Schedule.whileInput(isRateLimitError),
  Schedule.intersect(Schedule.recurs(5)),
);

class DbError {
  readonly _tag = "DbError";
  constructor(readonly message: string) {}
}

const fetchRecentlySyncedCustomers = (hours: number) =>
  Effect.gen(function* () {
    if (hours <= 0) return new Set<string>();

    const cutoff = new Date(Date.now() - hours * 60 * 60 * 1000).toISOString();

    const result = yield* Effect.tryPromise({
      try: () =>
        pool.query<{ id: string }>(
          `SELECT id FROM stripe.customers WHERE last_synced_at >= $1`,
          [cutoff],
        ),
      catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
    }).pipe(
      Effect.catchAll((e) =>
        Effect.gen(function* () {
          yield* Effect.logWarning(
            `Failed to fetch recently synced customers: ${e.message}`,
          );
          return { rows: [] as { id: string }[] };
        }),
      ),
    );

    return new Set(result.rows.map((c) => c.id));
  });

const fetchCustomersToSync = Effect.gen(function* () {
  const [subscriptionsResult, entitlementsResult, recentlySynced] =
    yield* Effect.all([
      Effect.tryPromise({
        try: () =>
          pool.query<{ customer: string }>(
            `SELECT customer FROM stripe.subscriptions WHERE status IN ('active', 'trialing', 'past_due')`,
          ),
        catch: (e) =>
          new DbError(
            `Failed to fetch subscriptions: ${e instanceof Error ? e.message : String(e)}`,
          ),
      }),
      Effect.tryPromise({
        try: () =>
          pool.query<{ customer: string }>(
            `SELECT customer FROM stripe.active_entitlements`,
          ),
        catch: (e) =>
          new DbError(
            `Failed to fetch existing entitlements: ${e instanceof Error ? e.message : String(e)}`,
          ),
      }),
      fetchRecentlySyncedCustomers(skipRecentHours),
    ]);

  const uniqueIds = new Set([
    ...subscriptionsResult.rows.map((s) => s.customer).filter(Boolean),
    ...entitlementsResult.rows.map((e) => e.customer).filter(Boolean),
  ]);

  const filtered = Array.from(uniqueIds).filter(
    (id) => !recentlySynced.has(id),
  );
  const skipped = uniqueIds.size - filtered.length;

  if (skipped > 0) {
    yield* Effect.log(
      `Skipping ${skipped} customers synced within the last ${skipRecentHours} hours`,
    );
  }

  return filtered;
});

const fetchCustomerEntitlements = (customerId: string) =>
  Effect.tryPromise({
    try: async () => {
      const entitlements: Stripe.Entitlements.ActiveEntitlement[] = [];
      for await (const entitlement of stripe.entitlements.activeEntitlements.list(
        {
          customer: customerId,
        },
      )) {
        entitlements.push(entitlement);
      }
      return entitlements;
    },
    catch: (error) => error,
  }).pipe(Effect.retry(retryPolicy));

const deleteAllEntitlements = (customerId: string) =>
  Effect.tryPromise({
    try: () =>
      pool.query(`DELETE FROM stripe.active_entitlements WHERE customer = $1`, [
        customerId,
      ]),
    catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
  }).pipe(
    Effect.map((result) => ({
      updated: 0,
      deleted: result.rowCount ?? 0,
      hasError: false,
    })),
    Effect.catchAll((e) =>
      Effect.gen(function* () {
        yield* Effect.logError(
          `Failed to delete entitlements for ${customerId}: ${e.message}`,
        );
        return { updated: 0, deleted: 0, hasError: true };
      }),
    ),
  );

const syncEntitlements = (
  customerId: string,
  entitlements: Stripe.Entitlements.ActiveEntitlement[],
) =>
  Effect.gen(function* () {
    const activeLookupKeys = entitlements.map((e) => e.lookup_key);

    const deleteResult = yield* Effect.tryPromise({
      try: () =>
        pool.query(
          `DELETE FROM stripe.active_entitlements WHERE customer = $1 AND lookup_key != ALL($2)`,
          [customerId, activeLookupKeys],
        ),
      catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
    }).pipe(
      Effect.catchAll((e) =>
        Effect.gen(function* () {
          yield* Effect.logError(
            `Failed to delete stale entitlements for ${customerId}: ${e.message}`,
          );
          return null;
        }),
      ),
    );

    if (deleteResult === null) {
      return { updated: 0, deleted: 0, hasError: true };
    }

    const deleteCount = deleteResult.rowCount ?? 0;

    for (const entitlement of entitlements) {
      const upsertResult = yield* Effect.tryPromise({
        try: () =>
          pool.query(
            `INSERT INTO stripe.active_entitlements (id, object, livemode, feature, customer, lookup_key, last_synced_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (customer, lookup_key) DO UPDATE SET
               id = EXCLUDED.id,
               object = EXCLUDED.object,
               livemode = EXCLUDED.livemode,
               feature = EXCLUDED.feature,
               last_synced_at = EXCLUDED.last_synced_at`,
            [
              entitlement.id,
              entitlement.object,
              entitlement.livemode,
              entitlement.feature,
              customerId,
              entitlement.lookup_key,
              new Date().toISOString(),
            ],
          ),
        catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
      }).pipe(
        Effect.catchAll((e) =>
          Effect.gen(function* () {
            yield* Effect.logError(
              `Failed to upsert entitlement for ${customerId}: ${e.message}`,
            );
            return null;
          }),
        ),
      );

      if (upsertResult === null) {
        return { updated: 0, deleted: deleteCount, hasError: true };
      }
    }

    return {
      updated: entitlements.length,
      deleted: deleteCount,
      hasError: false,
    };
  });

const updateCustomerLastSyncedAt = (customerId: string) =>
  Effect.tryPromise({
    try: () =>
      pool.query(
        `UPDATE stripe.customers SET last_synced_at = $1 WHERE id = $2`,
        [new Date().toISOString(), customerId],
      ),
    catch: (e) => new DbError(e instanceof Error ? e.message : String(e)),
  }).pipe(Effect.catchAll(() => Effect.void));

const processCustomer = (customerId: string) =>
  Effect.gen(function* () {
    const entitlements = yield* fetchCustomerEntitlements(customerId);

    const result =
      entitlements.length === 0
        ? yield* deleteAllEntitlements(customerId)
        : yield* syncEntitlements(customerId, entitlements);

    if (!result.hasError) {
      yield* updateCustomerLastSyncedAt(customerId);
    }

    return result;
  }).pipe(
    Effect.catchAll((error) =>
      Effect.gen(function* () {
        yield* Effect.logError(
          `Failed to process customer ${customerId}: ${error}`,
        );
        return { updated: 0, deleted: 0, hasError: true };
      }),
    ),
  );

const program = Effect.gen(function* () {
  yield* Effect.log("Starting Stripe entitlements sync...");
  yield* Effect.log(
    "Fetching customers with active subscriptions or existing entitlements...",
  );

  const customerIds = yield* fetchCustomersToSync;

  yield* Effect.log(`Found ${customerIds.length} customers to process`);

  let processed = 0;
  let totalUpdated = 0;
  let totalDeleted = 0;
  let totalErrors = 0;

  for (const customerId of customerIds) {
    const result = yield* processCustomer(customerId);
    processed++;
    totalUpdated += result.updated ?? 0;
    totalDeleted += result.deleted;
    if (result.hasError) totalErrors++;

    if (processed % 100 === 0) {
      yield* Effect.log(`Progress: ${processed}/${customerIds.length}`);
    }
  }

  yield* Effect.log(
    `Sync complete: processed=${processed}, updated=${totalUpdated}, deleted=${totalDeleted}, errors=${totalErrors}`,
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
