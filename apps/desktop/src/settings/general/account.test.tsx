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
  useBillingAccess: () => ({
    canStartTrial: false,
    isPaid: false,
    isTrialing: false,
    plan: "free",
    trialDaysRemaining: null,
  }),
}));

import { SettingsAccount } from "./account";

describe("SettingsAccount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  });

  afterEach(cleanup);

  it("confirms sign-out before ending the session", async () => {
    const queryClient = new QueryClient({
      defaultOptions: {
        mutations: { retry: false },
        queries: { retry: false },
      },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <SettingsAccount />
      </QueryClientProvider>,
    );

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
});
