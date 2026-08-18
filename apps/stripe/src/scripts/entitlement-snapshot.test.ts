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
  let released = 0;
  let connects = 0;

  const client: SnapshotClient = {
    async query(sql: string, params?: unknown[]) {
      const call = calls.length;
      calls.push({ sql, params });
      if (options.failOn?.(sql, call)) {
        throw new Error(`injected failure: ${sql.slice(0, 24)}`);
      }
      return { rowCount: sql.startsWith("DELETE") ? 2 : 1 };
    },
    release() {
      released++;
    },
  };

  const pool: SnapshotPool = {
    async connect() {
      connects++;
      return client;
    },
  };

  return {
    pool,
    calls,
    kinds: () => calls.map(({ sql }) => sql.split(/[ \n]/, 1)[0]),
    released: () => released,
    connects: () => connects,
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
  expect(db.released()).toBe(1);
});

test("empty snapshots use the same transaction path and delete everything", async () => {
  const db = fakePool();

  const result = await applyEntitlementSnapshot(db.pool, "cus_1", []);

  expect(db.kinds()).toEqual(["BEGIN", "DELETE", "UPDATE", "COMMIT"]);
  expect(db.calls[1]?.params).toEqual(["cus_1", []]);
  expect(result).toEqual({ updated: 0, deleted: 2, hasError: false });
});

test("rolls back when stale deletion fails", async () => {
  const db = fakePool({ failOn: (sql) => sql.startsWith("DELETE") });

  await expect(
    applyEntitlementSnapshot(db.pool, "cus_1", [entitlement("pro")]),
  ).rejects.toThrow("injected failure");

  expect(db.kinds()).toEqual(["BEGIN", "DELETE", "ROLLBACK"]);
  expect(db.released()).toBe(1);
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
});

test("rolls back when the last_synced_at update fails", async () => {
  const db = fakePool({ failOn: (sql) => sql.startsWith("UPDATE") });

  await expect(
    applyEntitlementSnapshot(db.pool, "cus_1", [entitlement("pro")]),
  ).rejects.toThrow("injected failure");

  expect(db.kinds()).toEqual([
    "BEGIN",
    "DELETE",
    "INSERT",
    "UPDATE",
    "ROLLBACK",
  ]);
  expect(db.kinds()).not.toContain("COMMIT");
});

test("releases the connection even when rollback itself fails", async () => {
  const db = fakePool({
    failOn: (sql) => sql.startsWith("DELETE") || sql.startsWith("ROLLBACK"),
  });

  await expect(
    applyEntitlementSnapshot(db.pool, "cus_1", [entitlement("pro")]),
  ).rejects.toThrow("injected failure: DELETE");

  expect(db.released()).toBe(1);
});

test("concurrent customers each get their own connection and transaction", async () => {
  const clients: Array<ReturnType<typeof fakePool>> = [];
  const pool: SnapshotPool = {
    async connect() {
      const db = fakePool();
      clients.push(db);
      return db.pool.connect();
    },
  };

  const [first, second] = await Promise.all([
    applyEntitlementSnapshot(pool, "cus_1", [entitlement("pro")]),
    applyEntitlementSnapshot(pool, "cus_2", []),
  ]);

  expect(first.hasError).toBe(false);
  expect(second.hasError).toBe(false);
  expect(clients).toHaveLength(2);
  for (const db of clients) {
    expect(db.kinds()[0]).toBe("BEGIN");
    expect(db.kinds().at(-1)).toBe("COMMIT");
  }
});
