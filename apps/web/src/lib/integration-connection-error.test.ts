import assert from "node:assert/strict";
import test from "node:test";

import {
  getNangoAuthErrorType,
  shouldReportConnectionAuthError,
} from "./integration-connection-error.ts";

test("reads Nango AuthError types and falls back for unknown values", () => {
  assert.equal(
    getNangoAuthErrorType({ type: "blocked_by_browser" }),
    "blocked_by_browser",
  );
  assert.equal(getNangoAuthErrorType(new Error("nope")), "unknown_error");
  assert.equal(getNangoAuthErrorType("window_closed"), "unknown_error");
});

test("does not report a closed sign-in window as an operational error", () => {
  assert.equal(shouldReportConnectionAuthError("window_closed"), false);
  assert.equal(shouldReportConnectionAuthError("blocked_by_browser"), true);
  assert.equal(shouldReportConnectionAuthError("unknown_error"), true);
});
