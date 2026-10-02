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
    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "This requires Anarlog Pro",
      {
        action: {
          label: "Upgrade",
          onClick: expect.any(Function),
        },
      },
    );

    mocks.toastWarning.mock.calls[0]?.[1].action.onClick();
    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
  });

  it.each([
    ["team", "Create workspace", "This requires Anarlog Team"],
    ["enterprise", "Require SSO", "This requires Anarlog Enterprise"],
  ] as const)(
    "toasts for %s without opening Pro checkout",
    (plan, label, message) => {
      render(
        <PlanGate plan={plan} allowed={false}>
          <button type="button">{label}</button>
        </PlanGate>,
      );

      fireEvent.click(screen.getByRole("button", { name: label }));

      expect(mocks.toastWarning).toHaveBeenCalledWith(message, {});
      expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
    },
  );
});
