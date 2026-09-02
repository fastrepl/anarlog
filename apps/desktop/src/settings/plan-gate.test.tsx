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
  sonnerToast: { warning: mocks.toastWarning },
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

  it("toasts for Team without opening Pro checkout", () => {
    render(
      <PlanGate plan="team" allowed={false}>
        <button type="button">Create workspace</button>
      </PlanGate>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));

    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "This requires Anarlog Team",
      {},
    );
    expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
  });

  it("toasts for Enterprise without opening Pro checkout", () => {
    render(
      <PlanGate plan="enterprise" allowed={false}>
        <button type="button">Require SSO</button>
      </PlanGate>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Require SSO" }));

    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "This requires Anarlog Enterprise",
      {},
    );
    expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
  });
});
