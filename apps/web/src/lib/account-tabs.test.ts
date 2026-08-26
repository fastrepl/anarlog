import assert from "node:assert/strict";
import test from "node:test";

import {
  accountTabForSection,
  resolveAccountTab,
  sectionsForAccountTab,
} from "./account-tabs.ts";

test("maps section hashes to the tab that contains them", () => {
  assert.equal(accountTabForSection("plan"), "account");
  assert.equal(accountTabForSection("referrals"), "account");
  assert.equal(accountTabForSection("session"), "account");
  assert.equal(accountTabForSection("integrations"), "connections");
  assert.equal(accountTabForSection("shares"), "connections");
  assert.equal(accountTabForSection("api-keys"), "developer");
  assert.equal(accountTabForSection("missing"), undefined);
});

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

test("lists the sections for a tab in page order", () => {
  assert.deepEqual(
    sectionsForAccountTab("account").map((section) => section.id),
    ["profile", "plan", "referrals", "session", "danger"],
  );
  assert.deepEqual(
    sectionsForAccountTab("developer").map((section) => section.id),
    ["api-keys"],
  );
});
