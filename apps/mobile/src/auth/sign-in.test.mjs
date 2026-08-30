import assert from "node:assert/strict";
import test from "node:test";

import { buildSignInUrl } from "./sign-in.ts";

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
