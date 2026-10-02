import assert from "node:assert/strict";
import test from "node:test";

import {
  oauthProviderQueryParams,
  oauthProviderScopes,
} from "./oauth-provider.ts";

test("only Google and Microsoft sign-in receive an account chooser", () => {
  for (const provider of ["google", "azure"] as const) {
    assert.deepEqual(oauthProviderQueryParams(provider), {
      prompt: "select_account",
    });
  }
  for (const provider of ["apple", "github"] as const) {
    assert.equal(oauthProviderQueryParams(provider), undefined);
  }
});

test("only Microsoft login requests OIDC identity scopes", () => {
  assert.equal(oauthProviderScopes("azure"), "openid email profile");
  for (const provider of ["google", "apple", "github"] as const) {
    assert.equal(oauthProviderScopes(provider), undefined);
  }
});
