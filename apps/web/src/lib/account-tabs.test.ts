import assert from "node:assert/strict";
import test from "node:test";

import { resolveAccountTab } from "./account-tabs.ts";

test("prefers a section hash over the tab search param", () => {
  assert.equal(
    resolveAccountTab({ tab: "developer", hash: "#referrals" }),
    "account",
  );
  assert.equal(
    resolveAccountTab({ tab: "developer", hash: "#session" }),
    "account",
  );
  assert.equal(
    resolveAccountTab({ tab: "account", hash: "integrations" }),
    "connections",
  );
});

test("falls back to the tab param, then Account", () => {
  assert.equal(resolveAccountTab({ tab: "connections" }), "connections");
  assert.equal(resolveAccountTab({ tab: "notes" }), "notes");
  assert.equal(resolveAccountTab({ tab: "nope", hash: "" }), "account");
  assert.equal(resolveAccountTab({}), "account");
});

test("an empty hash after a tab click does not override the tab param", () => {
  assert.equal(
    resolveAccountTab({ tab: "connections", hash: "" }),
    "connections",
  );
  assert.equal(resolveAccountTab({ tab: "developer", hash: "#" }), "developer");
});
