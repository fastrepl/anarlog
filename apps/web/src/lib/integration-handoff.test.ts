import assert from "node:assert/strict";
import test from "node:test";

import {
  getNangoSessionToken,
  isDesktopIntegrationHandoff,
} from "./integration-handoff.ts";

test("recognizes a desktop Nango integration handoff", () => {
  assert.equal(
    isDesktopIntegrationHandoff({
      pathname: "/app/integration/",
      search: {
        flow: "desktop",
        action: "connect",
        handoff: "nango",
      },
    }),
    true,
  );
});

test("does not bypass app authentication for web or disconnect flows", () => {
  for (const search of [
    { flow: "web", action: "connect", handoff: "nango" },
    { flow: "desktop", action: "disconnect", handoff: "nango" },
    { flow: "desktop", action: "unknown", handoff: "nango" },
    { flow: "desktop", action: "connect" },
  ]) {
    assert.equal(
      isDesktopIntegrationHandoff({
        pathname: "/app/integration/",
        search,
      }),
      false,
    );
  }
});

test("reads the scoped Nango token from the URL fragment", () => {
  assert.equal(
    getNangoSessionToken("#session_token=nango%2Etoken%2Bvalue"),
    "nango.token+value",
  );
  assert.equal(getNangoSessionToken("#session_token="), null);
  assert.equal(
    getNangoSessionToken("#session_token=first&session_token=second"),
    null,
  );
  assert.equal(getNangoSessionToken("session_token=token"), null);
  assert.equal(getNangoSessionToken(""), null);
});
