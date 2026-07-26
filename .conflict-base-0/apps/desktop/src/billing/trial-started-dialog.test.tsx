import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { PRO_TRIAL_DAYS } from "@hypr/pricing";

import { TrialStartedDialog } from "./trial-started-dialog";

describe("TrialStartedDialog", () => {
  afterEach(cleanup);

  it("confirms automatic continuation for card-backed trials", () => {
    render(
      <TrialStartedDialog
        open
        onOpenChange={() => {}}
        trialDaysRemaining={PRO_TRIAL_DAYS}
        hasPaymentMethod
      />,
    );

    expect(screen.getByText(/continue automatically/)).toBeTruthy();
    expect(screen.queryByText(/Add a payment method/)).toBeNull();
  });

  it("asks cardless trial users to add a payment method", () => {
    render(
      <TrialStartedDialog
        open
        onOpenChange={() => {}}
        trialDaysRemaining={PRO_TRIAL_DAYS}
        hasPaymentMethod={false}
      />,
    );

    expect(screen.getByText(/Add a payment method/)).toBeTruthy();
  });

  it("uses the product trial duration while billing refreshes", () => {
    render(
      <TrialStartedDialog
        open
        onOpenChange={() => {}}
        trialDaysRemaining={null}
        hasPaymentMethod={false}
      />,
    );

    expect(screen.getByText(new RegExp(`${PRO_TRIAL_DAYS}-day`))).toBeTruthy();
  });
});
