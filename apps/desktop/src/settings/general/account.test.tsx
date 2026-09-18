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

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/shared/utils", () => ({
  buildWebAppUrl: mocks.buildWebAppUrl,
}));

vi.mock("./account-profile", () => ({
  AccountProfile: () => <div>Profile editor</div>,
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

    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));

    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(
      screen.getByRole("heading", { name: "Sign out of BlackMushi?" }),
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

  it("opens connected accounts on the website for the current app account", async () => {
    renderAccount();

    fireEvent.click(
      screen.getByRole("button", { name: "Manage sign-in methods" }),
    );

    await waitFor(() =>
      expect(mocks.buildWebAppUrl).toHaveBeenCalledWith("/app/account", {
        flow: "web",
        section: "connected-accounts",
        account_user_id: "user-1",
      }),
    );
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://anarlog.so/app/account#connected-accounts",
      null,
    );
  });

  it("keeps plans out of Account for members and guests", async () => {
    const view = renderAccount();
    expect(screen.queryByText("Plans")).toBeNull();
    expect(screen.queryByRole("button", { name: "Get Pro" })).toBeNull();
    expect(screen.getByText("Profile editor")).toBeTruthy();
    view.unmount();
    mocks.session = null;
    renderAccount();
    expect(screen.queryByText("Plans")).toBeNull();
    expect(screen.queryByRole("button", { name: "Get Pro" })).toBeNull();
    expect(screen.getByText("Profile editor")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Get started" }));
    await waitFor(() => expect(mocks.signIn).toHaveBeenCalledOnce());
    expect(mocks.openUrl).not.toHaveBeenCalled();
  });
});
