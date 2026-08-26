import assert from "node:assert/strict";
import test from "node:test";

import {
  mapSsoAuthError,
  normalizeSsoDomain,
  sessionUsesSso,
  SSO_REQUIRED_MESSAGE,
  SSO_UNAVAILABLE_MESSAGE,
} from "./sso-domain.ts";

function jwtWithAmr(methods: string[]) {
  const payload = Buffer.from(
    JSON.stringify({
      amr: methods.map((method) => ({ method, timestamp: 1 })),
    }),
  ).toString("base64url");
  return `eyJhbGciOiJub25lIn0.${payload}.`;
}

test("normalizeSsoDomain accepts a bare company domain", () => {
  assert.equal(normalizeSsoDomain("Acme.COM"), "acme.com");
});

test("normalizeSsoDomain extracts the domain from an email", () => {
  assert.equal(normalizeSsoDomain(" Jane.Doe@acme.com "), "acme.com");
});

test("normalizeSsoDomain strips a URL prefix", () => {
  assert.equal(
    normalizeSsoDomain("https://sso.acme.com/login"),
    "sso.acme.com",
  );
});

test("normalizeSsoDomain rejects incomplete hosts", () => {
  assert.equal(normalizeSsoDomain("acme"), null);
  assert.equal(normalizeSsoDomain("@acme"), null);
  assert.equal(normalizeSsoDomain(""), null);
});

test("mapSsoAuthError explains a missing Supabase SSO provider", () => {
  assert.equal(
    mapSsoAuthError("No SSO provider assigned for this domain"),
    SSO_UNAVAILABLE_MESSAGE,
  );
});

test("mapSsoAuthError hides a project-level SAML-disabled error", () => {
  assert.equal(
    mapSsoAuthError("SAML 2.0 is disabled"),
    SSO_UNAVAILABLE_MESSAGE,
  );
});

test("mapSsoAuthError keeps require-SSO hook errors readable", () => {
  assert.equal(
    mapSsoAuthError("this organization requires SSO"),
    SSO_REQUIRED_MESSAGE,
  );
});

test("sessionUsesSso detects the current SSO provider", () => {
  assert.equal(
    sessionUsesSso({
      user: {
        app_metadata: { provider: "sso:11111111-1111-1111-1111-111111111111" },
      },
    }),
    true,
  );
  assert.equal(
    sessionUsesSso({
      user: { app_metadata: { provider: "google" } },
    }),
    false,
  );
});

test("sessionUsesSso reads SSO from the access-token AMR claim", () => {
  assert.equal(
    sessionUsesSso({
      access_token: jwtWithAmr(["sso/saml"]),
      user: { app_metadata: { provider: "google" } },
    }),
    true,
  );
});
