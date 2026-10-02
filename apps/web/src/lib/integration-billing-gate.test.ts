import assert from "node:assert/strict";
import test from "node:test";

import { getIntegrationBillingGate } from "./integration-billing-gate.ts";

const verifiedGate = {
  action: "connect" as const,
  isBillingReady: true,
  isVerifying: false,
  verificationFailed: false,
};

test("billing gate stays closed until refreshed claims resolve", () => {
  const cases = [
    [{ ...verifiedGate, verifiedIsPaid: true }, "connect"],
    [{ ...verifiedGate, verifiedIsPaid: false }, "upgrade"],
    [
      { ...verifiedGate, isVerifying: true, verifiedIsPaid: undefined },
      "loading",
    ],
    [
      { ...verifiedGate, verificationFailed: true, verifiedIsPaid: undefined },
      "retry",
    ],
    [
      {
        action: "disconnect",
        isBillingReady: false,
        isVerifying: false,
        verificationFailed: false,
        verifiedIsPaid: undefined,
      },
      "disconnect",
    ],
  ] as const;

  for (const [input, expected] of cases) {
    assert.equal(getIntegrationBillingGate(input), expected);
  }
});
