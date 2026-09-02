import assert from "node:assert/strict";
import test from "node:test";

import { oauthProviderScopes } from "./oauth-provider.ts";

test("Microsoft login requests OIDC identity scopes", () => {
  assert.equal(oauthProviderScopes("azure"), "openid email profile");
});

test("Google login uses provider defaults", () => {
  assert.equal(oauthProviderScopes("google"), undefined);
});

test("Apple login uses provider defaults", () => {
  assert.equal(oauthProviderScopes("apple"), undefined);
});

test("GitHub only requests repo when reconnecting for admin access", () => {
  assert.equal(oauthProviderScopes("github"), undefined);
  assert.equal(oauthProviderScopes("github", true), "repo");
});
