import assert from "node:assert/strict";
import test from "node:test";

import {
  oauthProviderQueryParams,
  oauthProviderScopes,
} from "./oauth-provider.ts";

test("Google and Microsoft sign-in let the user choose an account", () => {
  for (const provider of ["google", "azure"] as const) {
    assert.deepEqual(oauthProviderQueryParams(provider), {
      prompt: "select_account",
    });
  }
});

test("other providers do not receive unsupported account chooser parameters", () => {
  for (const provider of ["apple", "github"] as const) {
    assert.equal(oauthProviderQueryParams(provider), undefined);
  }
});

test("Microsoft login requests OIDC identity scopes", () => {
  assert.equal(oauthProviderScopes("azure"), "openid email profile");
});

test("Google login uses provider defaults", () => {
  assert.equal(oauthProviderScopes("google"), undefined);
});

test("Apple login uses provider defaults", () => {
  assert.equal(oauthProviderScopes("apple"), undefined);
});

test("GitHub login uses provider defaults", () => {
  assert.equal(oauthProviderScopes("github"), undefined);
});
