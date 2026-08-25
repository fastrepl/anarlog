import assert from "node:assert/strict";
import test from "node:test";

import { canUseMobileCapture } from "./capture-access.ts";

test("blocks capture until encrypted sync has been enrolled", () => {
  assert.equal(
    canUseMobileCapture(
      {
        phase: "setup_required",
        accountUserId: "user-123",
        hasRecoveryKey: false,
      },
      "user-123",
    ),
    false,
  );
  assert.equal(
    canUseMobileCapture(
      {
        phase: "starting",
        accountUserId: "user-123",
        hasRecoveryKey: false,
      },
      "user-123",
    ),
    false,
  );
  assert.equal(
    canUseMobileCapture(
      {
        phase: "ready",
        accountUserId: "user-123",
        hasRecoveryKey: false,
      },
      "user-123",
    ),
    false,
  );
});

test("keeps capture available through an ordinary outage after enrollment", () => {
  for (const phase of ["inactive", "starting", "ready", "error"]) {
    assert.equal(
      canUseMobileCapture(
        { phase, accountUserId: "user-123", hasRecoveryKey: true },
        "user-123",
      ),
      true,
    );
  }
});

test("does not carry enrollment across accounts", () => {
  assert.equal(
    canUseMobileCapture(
      {
        phase: "inactive",
        accountUserId: "user-123",
        hasRecoveryKey: true,
      },
      "user-456",
    ),
    false,
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
    assert.equal(
      canUseMobileCapture(
        { phase, accountUserId: "user-123", hasRecoveryKey: true },
        "user-123",
      ),
      false,
    );
  }
});
