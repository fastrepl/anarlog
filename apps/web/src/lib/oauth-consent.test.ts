import assert from "node:assert/strict";
import test from "node:test";

import { describeOAuthScopes } from "./oauth-consent.ts";

test("OAuth scopes use clear user-facing descriptions", () => {
  assert.deepEqual(describeOAuthScopes("openid email"), [
    "Confirm your Anarlog account identity",
    "Share your Anarlog account email with the connector",
  ]);
});

test("OAuth scope descriptions preserve unknown scopes", () => {
  assert.deepEqual(describeOAuthScopes("openid future_scope"), [
    "Confirm your Anarlog account identity",
    "Grant the future_scope permission",
  ]);
});
