import assert from "node:assert/strict";
import test from "node:test";

import {
  filterInvalidSupabaseCookies,
  toSetCookieOptions,
} from "./supabase-cookies.ts";

test("drops all chunks of a malformed base64 auth cookie", () => {
  const cookies = [
    { name: "sb-example-auth-token.0", value: "base64-eyJmb28iOiJiYXIifQ" },
    { name: "sb-example-auth-token.1", value: "base64-_w" },
    { name: "unrelated", value: "base64-_w" },
  ];

  assert.deepEqual(filterInvalidSupabaseCookies(cookies), [cookies[2]]);
});

test("preserves valid base64 cookies and raw cookies", () => {
  const cookies = [
    { name: "sb-example-auth-token", value: "base64-eyJmb28iOiJiYXIifQ" },
    { name: "sb-example-code-verifier", value: "verifier" },
  ];

  assert.deepEqual(filterInvalidSupabaseCookies(cookies), cookies);
});

test("forwards PKCE cookie options onto the document", () => {
  const options = toSetCookieOptions({
    name: "sb-auth-code-verifier",
    value: "verifier",
    options: {
      httpOnly: true,
      maxAge: 3600,
      path: "/",
      sameSite: "lax",
      secure: true,
    },
  });

  assert.deepEqual(options, {
    domain: undefined,
    expires: undefined,
    httpOnly: true,
    maxAge: 3600,
    path: "/",
    sameSite: "lax",
    secure: true,
  });
});

test("defaults the cookie path so OAuth redirects still send the verifier", () => {
  const options = toSetCookieOptions({
    name: "sb-auth-code-verifier",
    value: "verifier",
  });

  assert.equal(options.path, "/");
  assert.equal(options.sameSite, undefined);
});
