import assert from "node:assert/strict";
import test from "node:test";

import {
  connectionIdentityLabel,
  connectionNeedsReconnect,
} from "./integration-connection-label.ts";

test("treats reconnect_required or a stored error as a broken connection", () => {
  assert.equal(
    connectionNeedsReconnect({ status: "reconnect_required" }),
    true,
  );
  assert.equal(
    connectionNeedsReconnect({ last_error_type: "refresh_failed" }),
    true,
  );
  assert.equal(connectionNeedsReconnect({ status: "connected" }), false);
});

test("prefers the connected account or workspace over Connected copy", () => {
  assert.equal(
    connectionIdentityLabel({
      account_identity: "john@fastrepl.com",
      status: "connected",
    }),
    "john@fastrepl.com",
  );
  assert.equal(
    connectionIdentityLabel({
      account_identity: " Fastrepl ",
      status: "reconnect_required",
    }),
    "Fastrepl",
  );
});
