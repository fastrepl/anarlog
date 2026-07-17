import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { TrialStartedDialog } from "./trial-started-dialog";

describe("TrialStartedDialog", () => {
  afterEach(cleanup);

  it("confirms automatic continuation for card-backed trials", () => {
    render(
      <TrialStartedDialog
        open
        onOpenChange={() => {}}
        trialDaysRemaining={14}
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
        trialDaysRemaining={14}
        hasPaymentMethod={false}
      />,
    );

    expect(screen.getByText(/Add a payment method/)).toBeTruthy();
  });
});
