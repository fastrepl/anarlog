import assert from "node:assert/strict";
import test from "node:test";

import { toSetCookieOptions } from "./supabase-cookies.ts";

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
