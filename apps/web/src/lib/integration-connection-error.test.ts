import assert from "node:assert/strict";
import test from "node:test";

import {
  getConnectionErrorMessage,
  getNangoAuthErrorType,
} from "./integration-connection-error.ts";

test("reads Nango AuthError types and falls back for unknown values", () => {
  assert.equal(
    getNangoAuthErrorType({ type: "blocked_by_browser" }),
    "blocked_by_browser",
  );
  assert.equal(getNangoAuthErrorType(new Error("nope")), "unknown_error");
  assert.equal(getNangoAuthErrorType("window_closed"), "unknown_error");
});

test("explains Google’s blocked-app page when Calendar connect is closed", () => {
  assert.match(
    getConnectionErrorMessage(
      "window_closed",
      "Google Calendar",
      "google-calendar",
    ),
    /This app is blocked/,
  );
});

test("keeps Outlook window-closed copy generic", () => {
  assert.equal(
    getConnectionErrorMessage("window_closed", "Outlook Calendar", "outlook"),
    "The Outlook Calendar sign-in window closed before the connection finished. Please try again.",
  );
});

test("asks users to allow pop-ups when the browser blocks the window", () => {
  assert.match(
    getConnectionErrorMessage(
      "blocked_by_browser",
      "Google Calendar",
      "google-calendar",
    ),
    /Allow pop-ups/,
  );
});
