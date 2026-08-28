import assert from "node:assert/strict";
import test from "node:test";

import {
  describeOAuthScopes,
  oauthAuthorizationIdSchema,
} from "./oauth-consent.ts";

test("OAuth authorization IDs accept Supabase opaque identifiers", () => {
  assert.equal(
    oauthAuthorizationIdSchema.parse("2golbs6lfj6pkquumjxxlhplfrpugele"),
    "2golbs6lfj6pkquumjxxlhplfrpugele",
  );
  assert.throws(() => oauthAuthorizationIdSchema.parse("invalid/id"));
});

test("OAuth scopes use clear user-facing descriptions", () => {
  assert.deepEqual(describeOAuthScopes("openid email offline_access"), [
    "Confirm your Anarlog account identity",
    "Share your Anarlog account email with the connector",
    "Stay connected without asking you to sign in every time",
  ]);
});

test("OAuth scope descriptions preserve unknown scopes", () => {
  assert.deepEqual(describeOAuthScopes("openid future_scope"), [
    "Confirm your Anarlog account identity",
    "Grant the future_scope permission",
  ]);
});
