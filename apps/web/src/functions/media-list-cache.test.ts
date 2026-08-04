import assert from "node:assert/strict";
import test from "node:test";

import {
  BoundedInFlightRequests,
  getFreshCacheValue,
  setExpiringCacheValue,
  type ExpiringCacheEntry,
} from "./media-list-cache.ts";

function deferred<T>() {
  let resolve = (_value: T) => {};
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

test("globally removes expired entries during unrelated reads", () => {
  const cache = new Map<string, ExpiringCacheEntry<string>>([
    ["expired-a", { expiresAt: 99, value: "a" }],
    ["expired-b", { expiresAt: 100, value: "b" }],
    ["fresh", { expiresAt: 101, value: "fresh" }],
  ]);

  assert.equal(getFreshCacheValue(cache, "missing", 100, 10), undefined);
  assert.deepEqual([...cache.keys()], ["fresh"]);
});

test("caps entries and evicts the least recently used value", () => {
  const cache = new Map<string, ExpiringCacheEntry<string>>();
  const now = 100;
  setExpiringCacheValue(cache, "a", "a", now, 1_000, 2);
  setExpiringCacheValue(cache, "b", "b", now, 1_000, 2);

  assert.equal(getFreshCacheValue(cache, "a", now, 2), "a");
  setExpiringCacheValue(cache, "c", "c", now, 1_000, 2);

  assert.deepEqual([...cache.keys()], ["a", "c"]);
});

test("deduplicates in-flight work for the same key", async () => {
  const inFlight = new BoundedInFlightRequests<string>(2);
  let loads = 0;
  const load = async () => {
    loads += 1;
    return "loaded";
  };

  const first = inFlight.getOrStart("path", load);
  const second = inFlight.getOrStart("path", load);

  assert.equal(first, second);
  assert.equal(await first, "loaded");
  assert.equal(loads, 1);
});

test("caps concurrent work across unique keys", async () => {
  const inFlight = new BoundedInFlightRequests<string>(1);
  const firstLoad = deferred<string>();
  const first = inFlight.getOrStart("first", () => firstLoad.promise);

  assert.equal(
    inFlight.getOrStart("second", async () => "second"),
    undefined,
  );
  firstLoad.resolve("first");
  assert.equal(await first, "first");
  assert.equal(
    await inFlight.getOrStart("second", async () => "second"),
    "second",
  );
});

test("settled invalidated work does not delete its replacement", async () => {
  const inFlight = new BoundedInFlightRequests<string>(2);
  const oldLoad = deferred<string>();
  const replacementLoad = deferred<string>();
  const oldRequest = inFlight.getOrStart("path", () => oldLoad.promise);

  inFlight.deleteWhere((key) => key === "path");
  const replacement = inFlight.getOrStart(
    "path",
    () => replacementLoad.promise,
  );

  oldLoad.resolve("old");
  assert.equal(await oldRequest, "old");
  assert.equal(inFlight.get("path"), replacement);

  replacementLoad.resolve("replacement");
  assert.equal(await replacement, "replacement");
  assert.equal(inFlight.get("path"), undefined);
});
