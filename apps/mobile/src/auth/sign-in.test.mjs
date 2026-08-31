import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSignInUrl,
  parseAuthCallbackSignInMethod,
  parseLastSignInMethod,
} from "./sign-in.ts";

for (const provider of ["apple", "google", "azure", "github"]) {
  test(`builds a direct ${provider} OAuth URL`, () => {
    const url = new URL(buildSignInUrl("https://anarlog.so/", provider));

    assert.equal(url.origin, "https://anarlog.so");
    assert.equal(url.pathname, "/auth");
    assert.equal(url.searchParams.get("flow"), "desktop");
    assert.equal(url.searchParams.get("scheme"), "anarlog");
    assert.equal(url.searchParams.get("provider"), provider);
    assert.equal(url.searchParams.has("view"), false);
  });
}

for (const view of ["email", "sso"]) {
  test(`builds a direct ${view} sign-in URL`, () => {
    const url = new URL(buildSignInUrl("https://anarlog.so", view));

    assert.equal(url.pathname, "/auth");
    assert.equal(url.searchParams.get("flow"), "desktop");
    assert.equal(url.searchParams.get("scheme"), "anarlog");
    assert.equal(url.searchParams.get("view"), view);
    assert.equal(url.searchParams.has("provider"), false);
  });
}

test("accepts only supported last-used sign-in methods", () => {
  for (const method of ["apple", "google", "azure", "github", "email", "sso"]) {
    assert.equal(parseLastSignInMethod(method), method);
  }

  assert.equal(parseLastSignInMethod("password"), null);
  assert.equal(parseLastSignInMethod(null), null);
});

test("reads the sign-in method carried by an auth callback", () => {
  assert.equal(
    parseAuthCallbackSignInMethod(
      "anarlog://auth/callback?access_token=token&method=google",
    ),
    "google",
  );
  assert.equal(
    parseAuthCallbackSignInMethod(
      "anarlog://auth/callback?access_token=token&method=password",
    ),
    null,
  );
  assert.equal(parseAuthCallbackSignInMethod("not a url"), null);
});
