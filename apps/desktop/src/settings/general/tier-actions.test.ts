import { describe, expect, it } from "vitest";

import { getActionForTier, type PlanTier } from "@anlg/pricing";

describe("getActionForTier", () => {
  const cases: Array<{
    tierId: PlanTier;
    currentPlan: PlanTier;
    canStartTrial: boolean;
    expected: ReturnType<typeof getActionForTier>;
  }> = [
    {
      tierId: "free",
      currentPlan: "free",
      canStartTrial: false,
      expected: { kind: "current" },
    },
    {
      tierId: "free",
      currentPlan: "free",
      canStartTrial: true,
      expected: { kind: "current" },
    },
    {
      tierId: "pro",
      currentPlan: "pro",
      canStartTrial: false,
      expected: { kind: "current" },
    },
    {
      tierId: "pro",
      currentPlan: "pro",
      canStartTrial: true,
      expected: { kind: "current" },
    },
    {
      tierId: "pro",
      currentPlan: "free",
      canStartTrial: true,
      expected: { kind: "startTrial", plan: "pro" },
    },
    {
      tierId: "pro",
      currentPlan: "free",
      canStartTrial: false,
      expected: { kind: "checkout", plan: "pro", direction: "upgrade" },
    },
    {
      tierId: "free",
      currentPlan: "pro",
      canStartTrial: false,
      expected: null,
    },
    {
      tierId: "free",
      currentPlan: "pro",
      canStartTrial: true,
      expected: null,
    },
  ];

  for (const { tierId, currentPlan, canStartTrial, expected } of cases) {
    it(`resolves ${currentPlan} -> ${tierId} (trial: ${canStartTrial})`, () => {
      expect(getActionForTier(tierId, currentPlan, canStartTrial)).toEqual(
        expected,
      );
    });
  }
});
