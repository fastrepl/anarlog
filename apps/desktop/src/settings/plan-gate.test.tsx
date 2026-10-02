import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  billing: {
    upgradeToPro: vi.fn(),
  },
  toastWarning: vi.fn(),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { warning: mocks.toastWarning },
}));

import { PlanGate } from "./plan-gate";

describe("PlanGate", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.billing.upgradeToPro.mockClear();
    mocks.toastWarning.mockClear();
  });

  it("lets allowed children handle clicks", () => {
    const onClick = vi.fn();

    render(
      <PlanGate plan="pro" allowed>
        <button type="button" onClick={onClick}>
          Enable
        </button>
      </PlanGate>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Enable" }));

    expect(onClick).toHaveBeenCalledOnce();
    expect(mocks.toastWarning).not.toHaveBeenCalled();
  });

  it("shows locked Pro controls and toasts instead of running them", () => {
    const onClick = vi.fn();

    render(
      <PlanGate plan="pro" allowed={false}>
        <button type="button" onClick={onClick}>
          Enable
        </button>
      </PlanGate>,
    );

    expect(screen.getByRole("button", { name: "Enable" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Enable" }));

    expect(onClick).not.toHaveBeenCalled();
    mocks.toastWarning.mock.calls[0]?.[1].action.onClick();
    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
  });

  it("blocks Enterprise controls without opening Pro checkout", () => {
    const onClick = vi.fn();
    render(
      <PlanGate plan="enterprise" allowed={false}>
        <button type="button" onClick={onClick}>
          Require SSO
        </button>
      </PlanGate>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Require SSO" }));

    expect(onClick).not.toHaveBeenCalled();
    expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
  });
});
