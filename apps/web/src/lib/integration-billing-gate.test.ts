import assert from "node:assert/strict";
import test from "node:test";

import { getIntegrationBillingGate } from "./integration-billing-gate.ts";

const verifiedGate = {
  action: "connect" as const,
  isBillingReady: true,
  isVerifying: false,
  verificationFailed: false,
};

test("uses refreshed paid claims before connecting", () => {
  assert.equal(
    getIntegrationBillingGate({
      ...verifiedGate,
      verifiedIsPaid: true,
    }),
    "connect",
  );
});

test("shows upgrade only after refreshed claims confirm a free plan", () => {
  assert.equal(
    getIntegrationBillingGate({
      ...verifiedGate,
      verifiedIsPaid: false,
    }),
    "upgrade",
  );
});

test("keeps the gate closed while billing claims refresh", () => {
  assert.equal(
    getIntegrationBillingGate({
      ...verifiedGate,
      isVerifying: true,
      verifiedIsPaid: undefined,
    }),
    "loading",
  );
});

test("offers a retry instead of showing a false paywall after refresh failure", () => {
  assert.equal(
    getIntegrationBillingGate({
      ...verifiedGate,
      verificationFailed: true,
      verifiedIsPaid: undefined,
    }),
    "retry",
  );
});

test("disconnect does not wait for billing verification", () => {
  assert.equal(
    getIntegrationBillingGate({
      action: "disconnect",
      isBillingReady: false,
      isVerifying: false,
      verificationFailed: false,
      verifiedIsPaid: undefined,
    }),
    "disconnect",
  );
});
