import assert from "node:assert/strict";
import test from "node:test";

import { signInRedirectSearch } from "./sign-in-redirect.ts";

test("the iOS app's marker survives the sign-in redirect", () => {
  assert.deepEqual(signInRedirectSearch("/app/account", { client: "ios" }), {
    flow: "web",
    redirect: "/app/account?client=ios",
    client: "ios",
  });
  assert.deepEqual(signInRedirectSearch("/app/account", {}), {
    flow: "web",
    redirect: "/app/account",
  });
});
