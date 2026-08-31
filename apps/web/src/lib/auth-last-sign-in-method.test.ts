import assert from "node:assert/strict";
import test from "node:test";

import {
  parseAuthSignInMethod,
  resolveSignInMethod,
  shouldRememberOtpSignIn,
} from "./auth-last-sign-in-method.ts";

test("accepts only supported sign-in methods", () => {
  for (const method of ["apple", "google", "azure", "github", "email", "sso"]) {
    assert.equal(parseAuthSignInMethod(method), method);
  }

  assert.equal(parseAuthSignInMethod("password"), null);
  assert.equal(parseAuthSignInMethod(null), null);
});

test("prefers the completed sign-in method over the account's original provider", () => {
  assert.equal(
    resolveSignInMethod({
      attemptedMethod: "email",
      provider: "google",
      usesSso: false,
    }),
    "email",
  );
  assert.equal(
    resolveSignInMethod({
      attemptedMethod: "google",
      provider: "email",
      usesSso: false,
    }),
    "google",
  );
});

test("falls back to authenticated session metadata for legacy callbacks", () => {
  assert.equal(
    resolveSignInMethod({ provider: "email", usesSso: true }),
    "sso",
  );
  assert.equal(
    resolveSignInMethod({ provider: "google", usesSso: false }),
    "google",
  );
});

test("does not replace the last sign-in method during account maintenance", () => {
  assert.equal(shouldRememberOtpSignIn("magiclink"), true);
  assert.equal(shouldRememberOtpSignIn("signup"), true);
  assert.equal(shouldRememberOtpSignIn("recovery"), false);
  assert.equal(shouldRememberOtpSignIn("email_change"), false);
});
