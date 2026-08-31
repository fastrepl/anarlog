import assert from "node:assert/strict";
import test from "node:test";

import {
  parseAuthSignInMethod,
  resolveSessionSignInMethod,
  shouldRememberOtpSignIn,
} from "./auth-last-sign-in-method.ts";

test("accepts only supported sign-in methods", () => {
  for (const method of ["apple", "google", "azure", "github", "email", "sso"]) {
    assert.equal(parseAuthSignInMethod(method), method);
  }

  assert.equal(parseAuthSignInMethod("password"), null);
  assert.equal(parseAuthSignInMethod(null), null);
});

test("prefers an authenticated SSO signal over the session provider", () => {
  assert.equal(
    resolveSessionSignInMethod({ provider: "email", usesSso: true }),
    "sso",
  );
  assert.equal(
    resolveSessionSignInMethod({ provider: "google", usesSso: false }),
    "google",
  );
});

test("does not replace the last sign-in method during account maintenance", () => {
  assert.equal(shouldRememberOtpSignIn("magiclink"), true);
  assert.equal(shouldRememberOtpSignIn("signup"), true);
  assert.equal(shouldRememberOtpSignIn("recovery"), false);
  assert.equal(shouldRememberOtpSignIn("email_change"), false);
});
