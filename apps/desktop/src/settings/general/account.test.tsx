import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyticsEvent: vi.fn(() => Promise.resolve()),
  analyticsSetProperties: vi.fn(() => Promise.resolve()),
  signIn: vi.fn(() => Promise.resolve()),
  signOut: vi.fn(() => Promise.resolve()),
  buildWebAppUrl: vi.fn(() => Promise.resolve("https://anarlog.so/app/portal")),
  billing: {
    canStartTrial: { data: false, isPending: false },
    hasPaymentMethod: false,
    isPaid: false,
    isTrialing: false,
    plan: "free",
    trialDaysRemaining: null as number | null,
  },
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    event: mocks.analyticsEvent,
    setProperties: mocks.analyticsSetProperties,
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: vi.fn() },
}));

vi.mock("@anlg/plugin-windows", () => ({
  openUrlWithInstruction: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    isRefreshingSession: false,
    refreshSession: vi.fn(),
    session: { user: { email: "john@example.com" } },
    signIn: mocks.signIn,
    signOut: mocks.signOut,
  }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/shared/utils", () => ({
  buildWebAppUrl: mocks.buildWebAppUrl,
}));

import { SettingsAccount } from "./account";

const renderAccount = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsAccount />
    </QueryClientProvider>,
  );
};

describe("SettingsAccount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: false,
      isPaid: false,
      isTrialing: false,
      plan: "free",
      trialDaysRemaining: null,
    };
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  });

  afterEach(cleanup);

  it("confirms sign-out before ending the session", async () => {
    renderAccount();

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(
      screen.getByRole("heading", { name: "Sign out of Anarlog?" }),
    ).toBeTruthy();
    expect(screen.getByRole("dialog").className).toContain("max-w-[320px]");

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
    const signOutButtons = screen.getAllByRole("button", { name: "Sign out" });
    fireEvent.click(signOutButtons[signOutButtons.length - 1]!);

    await waitFor(() => expect(mocks.signOut).toHaveBeenCalledOnce());
    expect(mocks.analyticsEvent).toHaveBeenCalledWith({
      event: "user_signed_out",
    });
  });

  it("offers to add a payment method during a cardless trial", async () => {
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: false,
      isPaid: true,
      isTrialing: true,
      plan: "trial",
      trialDaysRemaining: 3,
    };

    renderAccount();

    expect(screen.queryByText("Cancel")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Add payment method" }));

    await waitFor(() =>
      expect(mocks.buildWebAppUrl).toHaveBeenCalledWith("/app/portal", {
        intent: "payment_method_update",
      }),
    );
    expect(mocks.analyticsEvent).toHaveBeenCalledWith({
      event: "trial_payment_method_clicked",
      days_remaining: 3,
      source: "settings",
    });
  });

  it("keeps the manage-plan control once the trial has a payment method", () => {
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: true,
      isPaid: true,
      isTrialing: true,
      plan: "trial",
      trialDaysRemaining: 3,
    };

    renderAccount();

    expect(
      screen.queryByRole("button", { name: "Add payment method" }),
    ).toBeNull();
    expect(screen.getByText("Current plan")).toBeTruthy();
  });
});
