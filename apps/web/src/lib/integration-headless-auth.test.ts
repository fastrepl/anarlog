import assert from "node:assert/strict";
import test from "node:test";

import {
  isConnectSessionFailed,
  usesHeadlessOAuth,
} from "./integration-headless-auth.ts";

test("skips Connect UI for OAuth integrations that need no extra inputs", () => {
  assert.equal(usesHeadlessOAuth("outlook"), true);
  assert.equal(usesHeadlessOAuth("google-calendar"), true);
  assert.equal(usesHeadlessOAuth("slack"), true);
  assert.equal(usesHeadlessOAuth("notion"), true);
  assert.equal(usesHeadlessOAuth("unknown-api-key"), false);
});

test("keeps a cached connect session usable after a later refresh fails", () => {
  assert.equal(
    isConnectSessionFailed({
      isError: true,
      token: "cached-session",
    }),
    false,
  );
  assert.equal(
    isConnectSessionFailed({
      handedOffToken: "desktop-session",
      isError: true,
    }),
    false,
  );
  assert.equal(isConnectSessionFailed({ isError: true }), true);
  assert.equal(isConnectSessionFailed({ isError: false }), false);
});
