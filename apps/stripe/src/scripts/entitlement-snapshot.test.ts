import { expect, test } from "bun:test";
import type Stripe from "stripe";

import {
  applyEntitlementSnapshot,
  type SnapshotClient,
  type SnapshotPool,
} from "./entitlement-snapshot";

function entitlement(lookupKey: string): Stripe.Entitlements.ActiveEntitlement {
  return {
    id: `ent_${lookupKey}`,
    object: "entitlements.active_entitlement",
    livemode: false,
    feature: `feat_${lookupKey}`,
    lookup_key: lookupKey,
  } as Stripe.Entitlements.ActiveEntitlement;
}

function fakePool(
  options: { failOn?: (sql: string, call: number) => boolean } = {},
) {
  const calls: Array<{ sql: string; params?: unknown[] }> = [];
  const releases: Array<Error | undefined> = [];

  const client: SnapshotClient = {
    async query(sql: string, params?: unknown[]) {
      const call = calls.length;
      calls.push({ sql, params });
      if (options.failOn?.(sql, call)) {
        throw new Error(`injected failure: ${sql.slice(0, 24)}`);
      }
      return { rowCount: sql.startsWith("DELETE") ? 2 : 1 };
    },
    release(error?: Error) {
      releases.push(error);
    },
  };

  const pool: SnapshotPool = {
    async connect() {
      return client;
    },
  };

  return {
    pool,
    calls,
    releases,
    kinds: () => calls.map(({ sql }) => sql.split(/[ \n]/, 1)[0]),
  };
}

test("commits deletion, upserts, and last_synced_at in one transaction", async () => {
  const db = fakePool();

  const result = await applyEntitlementSnapshot(db.pool, "cus_1", [
    entitlement("pro"),
    entitlement("teams"),
  ]);

  expect(db.kinds()).toEqual([
    "BEGIN",
    "DELETE",
    "INSERT",
    "INSERT",
    "UPDATE",
    "COMMIT",
  ]);
  expect(db.calls[1]?.params).toEqual(["cus_1", ["pro", "teams"]]);
  expect(db.calls[4]?.sql).toContain("stripe.customers SET last_synced_at");
  expect(result).toEqual({ updated: 2, deleted: 2, hasError: false });
  expect(db.releases).toEqual([undefined]);
});

test("empty snapshots use the same transaction path and delete everything", async () => {
  const db = fakePool();

  const result = await applyEntitlementSnapshot(db.pool, "cus_1", []);

  expect(db.kinds()).toEqual(["BEGIN", "DELETE", "UPDATE", "COMMIT"]);
  expect(db.calls[1]?.params).toEqual(["cus_1", []]);
  expect(result).toEqual({ updated: 0, deleted: 2, hasError: false });
});

test("rolls back a partially applied upsert batch", async () => {
  let inserts = 0;
  const db = fakePool({
    failOn: (sql) => sql.startsWith("INSERT") && ++inserts === 2,
  });

  await expect(
    applyEntitlementSnapshot(db.pool, "cus_1", [
      entitlement("pro"),
      entitlement("teams"),
    ]),
  ).rejects.toThrow("injected failure");

  expect(db.kinds()).toEqual([
    "BEGIN",
    "DELETE",
    "INSERT",
    "INSERT",
    "ROLLBACK",
  ]);
  expect(db.releases).toEqual([undefined]);
});

test("releases the connection even when rollback itself fails", async () => {
  const db = fakePool({
    failOn: (sql) => sql.startsWith("DELETE") || sql.startsWith("ROLLBACK"),
  });

  await expect(
    applyEntitlementSnapshot(db.pool, "cus_1", [entitlement("pro")]),
  ).rejects.toThrow("injected failure: DELETE");

  // A failed rollback must destroy the connection, not return it to the pool.
  expect(db.releases).toHaveLength(1);
  expect(db.releases[0]).toBeInstanceOf(Error);
});
