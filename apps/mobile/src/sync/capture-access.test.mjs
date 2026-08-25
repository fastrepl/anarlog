import assert from "node:assert/strict";
import test from "node:test";

import { canUseMobileCapture } from "./capture-access.ts";

test("blocks capture until encrypted sync has been enrolled", () => {
  assert.equal(
    canUseMobileCapture({ phase: "setup_required", hasRecoveryKey: false }),
    false,
  );
  assert.equal(
    canUseMobileCapture({ phase: "starting", hasRecoveryKey: false }),
    false,
  );
  assert.equal(
    canUseMobileCapture({ phase: "ready", hasRecoveryKey: false }),
    false,
  );
});

test("keeps capture available through an ordinary outage after enrollment", () => {
  assert.equal(
    canUseMobileCapture({ phase: "error", hasRecoveryKey: true }),
    true,
  );
  assert.equal(
    canUseMobileCapture({ phase: "starting", hasRecoveryKey: true }),
    true,
  );
});

test("blocks explicit account and entitlement failures", () => {
  for (const phase of [
    "account_mismatch",
    "device_limit",
    "identity_mismatch",
    "not_entitled",
    "reauth_required",
  ]) {
    assert.equal(canUseMobileCapture({ phase, hasRecoveryKey: true }), false);
  }
});
