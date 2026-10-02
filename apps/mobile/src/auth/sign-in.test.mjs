import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSignInUrl,
  isAppleSignInAvailable,
  parseAuthCallbackSignInMethod,
  parseLastSignInMethod,
} from "./sign-in.ts";

test("builds direct sign-in URLs for each method and build scheme", () => {
  for (const provider of ["apple", "google", "azure", "github"]) {
    const url = new URL(buildSignInUrl("https://anarlog.so/", provider));

    assert.equal(url.origin, "https://anarlog.so");
    assert.equal(url.pathname, "/auth");
    assert.equal(url.searchParams.get("flow"), "desktop");
    assert.equal(url.searchParams.get("scheme"), "anarlog");
    assert.equal(url.searchParams.get("provider"), provider);
    assert.equal(url.searchParams.has("view"), false);
  }

  for (const view of ["email", "sso"]) {
    const url = new URL(buildSignInUrl("https://anarlog.so", view));

    assert.equal(url.pathname, "/auth");
    assert.equal(url.searchParams.get("flow"), "desktop");
    assert.equal(url.searchParams.get("scheme"), "anarlog");
    assert.equal(url.searchParams.get("view"), view);
    assert.equal(url.searchParams.has("provider"), false);
  }

  const staging = new URL(
    buildSignInUrl("https://anarlog.so", "google", "anarlog-staging"),
  );
  assert.equal(staging.searchParams.get("scheme"), "anarlog-staging");
});

test("hides Sign in with Apple on Android", () => {
  assert.equal(isAppleSignInAvailable("android"), false);
  assert.equal(isAppleSignInAvailable("ios"), true);
  assert.equal(isAppleSignInAvailable("web"), true);
});

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
