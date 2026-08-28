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
  openUrl: vi.fn(() => Promise.resolve()),
  signIn: vi.fn(() => Promise.resolve()),
  signOut: vi.fn(() => Promise.resolve()),
  buildWebAppUrl: vi.fn((path: string) =>
    Promise.resolve(`https://anarlog.so${path}`),
  ),
  billing: {
    canStartTrial: { data: false, isPending: false },
    hasPaymentMethod: false,
    isPaid: false,
    isTrialing: false,
    isPaused: false,
    plan: "free",
    trialDaysRemaining: null as number | null,
  },
  session: { user: { email: "john@example.com" } } as {
    user: { email: string };
  } | null,
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: {
    event: mocks.analyticsEvent,
    setProperties: mocks.analyticsSetProperties,
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("@anlg/plugin-windows", () => ({
  openUrlWithInstruction: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    isRefreshingSession: false,
    refreshSession: vi.fn(),
    session: mocks.session,
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
    mocks.session = { user: { email: "john@example.com" } };
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: false,
      isPaid: false,
      isTrialing: false,
      isPaused: false,
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

  it("opens the account page to change the signed-in email", async () => {
    renderAccount();

    fireEvent.click(screen.getByRole("button", { name: "john@example.com" }));

    await waitFor(() =>
      expect(mocks.buildWebAppUrl).toHaveBeenCalledWith("/app/account"),
    );
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://anarlog.so/app/account",
      null,
    );
  });

  it("offers to add a payment method during a cardless trial", async () => {
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: false,
      isPaid: true,
      isTrialing: true,
      isPaused: false,
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

  it("offers to resume a paused cardless trial", async () => {
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: false,
      isPaid: false,
      isTrialing: false,
      isPaused: true,
      plan: "free",
      trialDaysRemaining: 0,
    };

    renderAccount();

    expect(screen.getByText("Your Pro trial has ended")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Resume" }));

    await waitFor(() =>
      expect(mocks.buildWebAppUrl).toHaveBeenCalledWith("/app/portal"),
    );
  });

  it("starts a cardless trial from the behavioral action variant", async () => {
    mocks.billing = {
      canStartTrial: { data: true, isPending: false },
      hasPaymentMethod: false,
      isPaid: false,
      isTrialing: false,
      isPaused: false,
      plan: "free",
      trialDaysRemaining: null,
    };

    renderAccount();

    fireEvent.click(screen.getByRole("button", { name: "Start free trial" }));

    await waitFor(() =>
      expect(mocks.buildWebAppUrl).toHaveBeenCalledWith("/app/checkout", {
        period: "monthly",
        trial: "true",
        source: "settings",
      }),
    );
    expect(mocks.analyticsEvent).toHaveBeenCalledWith({
      event: "trial_checkout_started",
      plan: "pro",
      period: "monthly",
      source: "settings",
    });
  });

  it("opens checkout for an upgrade when no trial is available", async () => {
    renderAccount();

    fireEvent.click(screen.getByRole("button", { name: "Get Pro" }));

    await waitFor(() =>
      expect(mocks.buildWebAppUrl).toHaveBeenCalledWith("/app/checkout", {
        plan: "pro",
        period: "monthly",
        source: "settings",
      }),
    );
    expect(mocks.analyticsEvent).toHaveBeenCalledWith({
      event: "upgrade_clicked",
      plan: "pro",
      period: "monthly",
      source: "settings",
    });
  });

  it("asks guests to sign in instead of opening checkout", async () => {
    mocks.session = null;

    renderAccount();

    expect(screen.queryByRole("button", { name: "Get Pro" })).toBeNull();
    expect(screen.getByText("Current plan")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Sign in for Pro" }));

    await waitFor(() => expect(mocks.signIn).toHaveBeenCalled());
    expect(mocks.buildWebAppUrl).not.toHaveBeenCalledWith(
      "/app/checkout",
      expect.anything(),
    );
  });

  it("renders the current plan as status once the trial has a payment method", () => {
    mocks.billing = {
      canStartTrial: { data: false, isPending: false },
      hasPaymentMethod: true,
      isPaid: true,
      isTrialing: true,
      isPaused: false,
      plan: "trial",
      trialDaysRemaining: 3,
    };

    renderAccount();

    expect(
      screen.queryByRole("button", { name: "Add payment method" }),
    ).toBeNull();
    expect(screen.getByText("Current plan")).toBeTruthy();
    expect(screen.queryByText("Cancel")).toBeNull();
    expect(screen.queryByRole("button", { name: /Current plan/ })).toBeNull();
  });
});
