import type Stripe from "stripe";

export type SnapshotResult = {
  updated: number;
  deleted: number;
  hasError: boolean;
};

type QueryResult = { rowCount: number | null };

export type SnapshotClient = {
  query(sql: string, params?: unknown[]): Promise<QueryResult>;
  // Passing an error destroys the connection instead of returning it to the
  // pool (node-postgres semantics).
  release(error?: Error): void;
};

export type SnapshotPool = {
  connect(): Promise<SnapshotClient>;
};

// Applies one customer's entitlement snapshot atomically: stale deletion,
// every upsert, and the successful last_synced_at update commit or roll back
// together. An empty snapshot takes the same path — `!= ALL('{}')` matches
// every row, so all of the customer's entitlements are deleted.
export async function applyEntitlementSnapshot(
  pool: SnapshotPool,
  customerId: string,
  entitlements: Stripe.Entitlements.ActiveEntitlement[],
): Promise<SnapshotResult> {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");

    const deleteResult = await client.query(
      `DELETE FROM stripe.active_entitlements WHERE customer = $1 AND lookup_key != ALL($2)`,
      [customerId, entitlements.map((e) => e.lookup_key)],
    );

    const now = new Date().toISOString();
    for (const entitlement of entitlements) {
      await client.query(
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
          now,
        ],
      );
    }

    await client.query(
      `UPDATE stripe.customers SET last_synced_at = $1 WHERE id = $2`,
      [now, customerId],
    );

    await client.query("COMMIT");
    client.release();
    return {
      updated: entitlements.length,
      deleted: deleteResult.rowCount ?? 0,
      hasError: false,
    };
  } catch (error) {
    try {
      await client.query("ROLLBACK");
      client.release();
    } catch (rollbackError) {
      // A failed rollback leaves the connection in an unknown state; destroy
      // it instead of returning a broken client to the pool.
      client.release(
        rollbackError instanceof Error
          ? rollbackError
          : new Error(String(rollbackError)),
      );
    }
    throw error;
  }
}
