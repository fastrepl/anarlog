import assert from "node:assert/strict";
import test from "node:test";

import {
  getWorkspaceShareSlug,
  isWorkspaceShareHostname,
} from "./workspace-share-host.ts";

test("accepts one valid enterprise workspace subdomain", () => {
  assert.equal(isWorkspaceShareHostname("fastrepl.anarlog.so"), true);
  assert.equal(isWorkspaceShareHostname("fastrepl-hq.anarlog.so"), true);
  assert.equal(getWorkspaceShareSlug("fastrepl.anarlog.so"), "fastrepl");
});

test("rejects reserved, nested, and malformed workspace hosts", () => {
  assert.equal(isWorkspaceShareHostname("api.anarlog.so"), false);
  assert.equal(isWorkspaceShareHostname("desktop.anarlog.so"), false);
  assert.equal(isWorkspaceShareHostname("models.anarlog.so"), false);
  assert.equal(isWorkspaceShareHostname("www.anarlog.so"), false);
  assert.equal(isWorkspaceShareHostname("a.b.anarlog.so"), false);
  assert.equal(isWorkspaceShareHostname("-fastrepl.anarlog.so"), false);
  assert.equal(isWorkspaceShareHostname("fastrepl.example.com"), false);
  assert.equal(getWorkspaceShareSlug("www.anarlog.so"), null);
});
