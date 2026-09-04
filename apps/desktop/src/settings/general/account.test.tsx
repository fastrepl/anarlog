import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { completeDestructiveButtonHold } from "~/test-utils/destructive-button";

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
  session: { user: { id: "user-1", email: "john@example.com" } } as {
    user: { id: string; email: string };
  } | null,
  workspaces: {
    data: [] as Array<{ workspaceId: string }>,
    isPending: false,
  },
  getWorkspaceAccess: vi.fn(),
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
    supabase: {},
    signIn: mocks.signIn,
    signOut: mocks.signOut,
  }),
}));

vi.mock("~/settings/team/client", () => ({
  getWorkspaceAccess: mocks.getWorkspaceAccess,
  requireTeamContext: (auth: unknown) => auth,
}));

vi.mock("~/settings/team/mirror", () => ({
  useMyWorkspacesWithMirror: () => mocks.workspaces,
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
    mocks.session = { user: { id: "user-1", email: "john@example.com" } };
    mocks.workspaces.data = [];
    mocks.workspaces.isPending = false;
    mocks.getWorkspaceAccess.mockResolvedValue({
      tier: "free",
      capabilities: [],
    });
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

    completeDestructiveButtonHold(
      screen.getByRole("button", { name: "Sign out" }),
    );

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(
      screen.getByRole("heading", { name: "Sign out of Anarlog?" }),
    ).toBeTruthy();
    expect(screen.getByRole("dialog").className).toContain("max-w-[320px]");

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();

    completeDestructiveButtonHold(
      screen.getByRole("button", { name: "Sign out" }),
    );
    const signOutButtons = screen.getAllByRole("button", { name: "Sign out" });
    completeDestructiveButtonHold(signOutButtons[signOutButtons.length - 1]!);

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

  it.each([
    { personalState: "trialing", isTrialing: true, isPaused: false },
    { personalState: "paused", isTrialing: false, isPaused: true },
  ])(
    "shows Team as current without the $personalState Pro status",
    async ({ isTrialing, isPaused }) => {
      mocks.billing = {
        canStartTrial: { data: false, isPending: false },
        hasPaymentMethod: true,
        isPaid: true,
        isTrialing,
        isPaused,
        plan: "pro",
        trialDaysRemaining: null,
      };
      mocks.workspaces.data = [
        { workspaceId: "00000000-0000-4000-8000-000000000001" },
      ];
      mocks.getWorkspaceAccess.mockResolvedValue({
        tier: "team",
        capabilities: ["team.shared_notes"],
      });

      renderAccount();

      expect(
        await screen.findByText(/You're on the .*Team.* plan/),
      ).toBeTruthy();
      expect(screen.queryByText("Your Pro trial has ended")).toBeNull();
      expect(screen.queryByText("Trial")).toBeNull();
      expect(
        screen.queryByRole("button", { name: "Manage billing" }),
      ).toBeNull();
      expect(screen.getAllByText("Current")).toHaveLength(1);
    },
  );

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

  it("shows all four offers and opens the Enterprise page", async () => {
    renderAccount();

    expect(screen.getByText("Free")).toBeTruthy();
    expect(screen.getByText("Pro")).toBeTruthy();
    expect(screen.getByText("Team")).toBeTruthy();
    expect(screen.getByText("Enterprise")).toBeTruthy();
    expect(screen.getByText("$20")).toBeTruthy();
    expect(screen.getByText("Custom")).toBeTruthy();
    expect(screen.queryByText("Soon")).toBeNull();
    expect(screen.getByText("Domain SSO and SCIM")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Teams" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Talk to sales" }));

    await waitFor(() =>
      expect(mocks.openUrl).toHaveBeenCalledWith(
        "https://anarlog.so/enterprise/",
        null,
      ),
    );
  });

  it("asks guests to sign in instead of opening checkout", async () => {
    mocks.session = null;

    renderAccount();

    expect(screen.queryByRole("button", { name: "Get Pro" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Sign in for Pro" }),
    ).toBeNull();
    expect(screen.getByText("Current")).toBeTruthy();
    expect(
      screen.getByText("Compare Free, Pro, Team, and Enterprise."),
    ).toBeTruthy();
    expect(screen.getByText("Cloud Transcription")).toBeTruthy();
    expect(screen.queryByText("On-device Transcription")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Get started" }));

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
    expect(screen.getByText("Current")).toBeTruthy();
    expect(screen.queryByText("Cancel")).toBeNull();
    expect(screen.queryByRole("button", { name: /Current/ })).toBeNull();
  });
});
