import assert from "node:assert/strict";
import test from "node:test";

import {
  addInternalReturnPathSearch,
  buildPostAuthDestination,
  DEFAULT_AUTH_RETURN_PATH,
  sanitizeInternalReturnPath,
  toAbsoluteInternalReturnUrl,
} from "./auth-redirect.ts";

test("accepts only internal paths with one leading slash", () => {
  assert.equal(
    sanitizeInternalReturnPath("/share/invite/abc/?scheme=hyprnote#note"),
    "/share/invite/abc/?scheme=hyprnote#note",
  );

  for (const value of [
    undefined,
    "",
    "share/invite/abc",
    "//attacker.example/share",
    "/\\attacker.example/share",
    "https://anarlog.so/share",
    "javascript:alert(1)",
  ]) {
    assert.equal(sanitizeInternalReturnPath(value), DEFAULT_AUTH_RETURN_PATH);
  }
});

test("new web accounts enter the card-required onboarding checkout", () => {
  const destination = buildPostAuthDestination({
    newAccount: true,
    returnTo: "/share/invite/abc/?scheme=hyprnote",
  });
  const url = new URL(destination, "https://anarlog.so");

  assert.equal(url.pathname, "/app/checkout");
  assert.equal(url.searchParams.get("period"), "monthly");
  assert.equal(url.searchParams.get("plan"), "pro");
  assert.equal(url.searchParams.get("trial"), "true");
  assert.equal(url.searchParams.get("source"), "onboarding");
  assert.equal(
    url.searchParams.get("return_to"),
    "/share/invite/abc/?scheme=hyprnote",
  );
});

test("returning users bypass checkout and unsafe redirects fall back", () => {
  assert.equal(
    buildPostAuthDestination({
      newAccount: false,
      returnTo: "/share/invite/abc/",
    }),
    "/share/invite/abc/",
  );
  assert.equal(
    buildPostAuthDestination({
      newAccount: false,
      returnTo: "//attacker.example",
    }),
    DEFAULT_AUTH_RETURN_PATH,
  );
});

test("checkout results preserve invitation query and hash state", () => {
  assert.equal(
    addInternalReturnPathSearch("/share/invite/abc/?scheme=hyprnote#note", {
      checkout: "canceled",
      checkout_type: "trial",
      source: "onboarding",
    }),
    "/share/invite/abc/?scheme=hyprnote&checkout=canceled&checkout_type=trial&source=onboarding#note",
  );
});

test("absolute Stripe return URLs cannot change the app origin", () => {
  assert.equal(
    toAbsoluteInternalReturnUrl(
      "https://anarlog.so",
      "/share/invite/abc/?scheme=hyprnote",
    ),
    "https://anarlog.so/share/invite/abc/?scheme=hyprnote",
  );
  assert.equal(
    toAbsoluteInternalReturnUrl(
      "https://anarlog.so",
      "//attacker.example/share",
    ),
    "https://anarlog.so/app/account/",
  );
});
