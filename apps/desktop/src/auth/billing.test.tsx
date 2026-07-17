import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { canStartTrial as canStartTrialApi } from "@hypr/api-client";
import { commands as authCommands } from "@hypr/plugin-auth";

import * as billingProviderModule from "./billing";
import { useBillingAccess } from "./billing-context";

const { BillingProvider } = billingProviderModule;

const refreshSession = vi.fn();
const authState = vi.hoisted(() => ({
  session: {
    access_token: "stale-token",
    user: { id: "user-1", email: "test@example.com" },
  },
}));

vi.mock("./auth-context", () => ({
  useAuth: () => ({
    session: authState.session,
    getHeaders: () => ({
      Authorization: `Bearer ${authState.session.access_token}`,
    }),
    refreshSession,
  }),
}));

vi.mock("@hypr/api-client", () => ({
  canStartTrial: vi.fn(),
}));

vi.mock("@hypr/api-client/client", () => ({
  createClient: vi.fn(() => ({})),
}));

vi.mock("@hypr/plugin-auth", () => ({
  commands: {
    decodeClaims: vi.fn(),
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openUrl: vi.fn(),
  },
}));

vi.mock("@hypr/plugin-windows", () => ({
  openUrlWithInstruction: vi.fn(),
}));

vi.mock("../billing/trial-ended-dialog", () => ({
  TrialEndedDialog: ({ open }: { open: boolean }) => (
    <div data-open={open ? "true" : "false"} data-testid="trial-ended-dialog" />
  ),
}));

vi.mock("../billing/trial-payment-reminder-dialog", () => ({
  TrialPaymentReminderDialog: ({
    open,
    daysRemaining,
  }: {
    open: boolean;
    daysRemaining: number;
  }) => (
    <div
      data-days-remaining={daysRemaining}
      data-open={open ? "true" : "false"}
      data-testid="trial-payment-reminder-dialog"
    />
  ),
}));

vi.mock("../billing/trial-started-dialog", () => ({
  TrialStartedDialog: ({ open }: { open: boolean }) => (
    <div
      data-open={open ? "true" : "false"}
      data-testid="trial-started-dialog"
    />
  ),
}));

function renderBillingProvider() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return {
    queryClient,
    view: render(billingTree(queryClient)),
  };
}

function billingTree(queryClient: QueryClient) {
  return (
    <QueryClientProvider client={queryClient}>
      <BillingProvider>
        <div>content</div>
        <BillingProbe />
      </BillingProvider>
    </QueryClientProvider>
  );
}

function BillingProbe() {
  const billing = useBillingAccess();
  return (
    <div
      data-is-paid={billing.isPaid ? "true" : "false"}
      data-is-ready={billing.isReady ? "true" : "false"}
      data-testid="billing-access"
    />
  );
}

function paidClaims(userId: string) {
  return {
    status: "ok" as const,
    data: {
      sub: userId,
      email: `${userId}@example.com`,
      entitlements: ["hyprnote_pro"],
      subscription_status: "active" as const,
      trial_end: null,
      has_payment_method: true,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe("BillingProvider", () => {
  it("keeps the provider module compatible with Fast Refresh", () => {
    expect(Object.keys(billingProviderModule)).toEqual(["BillingProvider"]);
  });

  beforeEach(() => {
    vi.stubGlobal("localStorage", {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
    });

    refreshSession.mockReset();
    authState.session = {
      access_token: "stale-token",
      user: { id: "user-1", email: "test@example.com" },
    };

    vi.mocked(authCommands.decodeClaims)
      .mockReset()
      .mockResolvedValue({
        status: "ok",
        data: {
          sub: "user-1",
          email: "test@example.com",
          entitlements: [],
          subscription_status: null,
          trial_end: null,
          has_payment_method: null,
        },
      });

    vi.mocked(canStartTrialApi).mockResolvedValue({
      data: { canStartTrial: false, reason: "not_eligible" as const },
      error: undefined,
      request: new Request("https://api.example.test/can-start-trial"),
      response: new Response(),
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("opens the trial-ended modal after a failed eligibility refresh", async () => {
    refreshSession.mockResolvedValue(null);

    renderBillingProvider();

    await waitFor(() => {
      expect(refreshSession).toHaveBeenCalledTimes(1);
    });

    await waitFor(() => {
      expect(
        screen.getByTestId("trial-ended-dialog").getAttribute("data-open"),
      ).toBe("true");
    });
  });

  it("keeps paid access while the same user's refreshed token is decoded", async () => {
    const refreshedClaims =
      deferred<Awaited<ReturnType<typeof authCommands.decodeClaims>>>();
    vi.mocked(authCommands.decodeClaims)
      .mockResolvedValueOnce(paidClaims("user-1"))
      .mockReturnValueOnce(refreshedClaims.promise);
    const { queryClient, view } = renderBillingProvider();

    await waitFor(() => {
      expect(
        screen.getByTestId("billing-access").getAttribute("data-is-paid"),
      ).toBe("true");
    });

    authState.session = {
      ...authState.session,
      access_token: "refreshed-token",
    };
    view.rerender(billingTree(queryClient));

    await waitFor(() => {
      expect(authCommands.decodeClaims).toHaveBeenCalledTimes(2);
    });
    expect(
      screen.getByTestId("billing-access").getAttribute("data-is-paid"),
    ).toBe("true");
    expect(
      screen.getByTestId("billing-access").getAttribute("data-is-ready"),
    ).toBe("true");

    refreshedClaims.resolve(paidClaims("user-1"));
  });

  it("does not retain paid access across account switches", async () => {
    const switchedClaims =
      deferred<Awaited<ReturnType<typeof authCommands.decodeClaims>>>();
    vi.mocked(authCommands.decodeClaims)
      .mockResolvedValueOnce(paidClaims("user-1"))
      .mockReturnValueOnce(switchedClaims.promise);
    const { queryClient, view } = renderBillingProvider();

    await waitFor(() => {
      expect(
        screen.getByTestId("billing-access").getAttribute("data-is-paid"),
      ).toBe("true");
    });

    authState.session = {
      access_token: "user-2-token",
      user: { id: "user-2", email: "user-2@example.com" },
    };
    view.rerender(billingTree(queryClient));

    await waitFor(() => {
      expect(authCommands.decodeClaims).toHaveBeenCalledTimes(2);
    });
    expect(
      screen.getByTestId("billing-access").getAttribute("data-is-paid"),
    ).toBe("false");
    expect(
      screen.getByTestId("billing-access").getAttribute("data-is-ready"),
    ).toBe("false");

    switchedClaims.resolve(paidClaims("user-2"));
  });

  it("opens a payment reminder during the final seven trial days", async () => {
    vi.mocked(localStorage.getItem).mockImplementation((key: string) =>
      key.startsWith("anarlog:trial_started_seen:") ? "1" : null,
    );
    vi.mocked(authCommands.decodeClaims).mockResolvedValue({
      status: "ok",
      data: {
        sub: "user-1",
        email: "test@example.com",
        entitlements: [],
        subscription_status: "trialing",
        trial_end: Math.floor(Date.now() / 1000) + 6 * 24 * 60 * 60,
        has_payment_method: false,
      },
    });

    renderBillingProvider();

    await waitFor(() => {
      const reminder = screen.getByTestId("trial-payment-reminder-dialog");
      expect(reminder.getAttribute("data-open")).toBe("true");
      expect(reminder.getAttribute("data-days-remaining")).toBe("6");
    });
  });

  it("does not remind trial users who already added a payment method", async () => {
    vi.mocked(localStorage.getItem).mockImplementation((key: string) =>
      key.startsWith("anarlog:trial_started_seen:") ? "1" : null,
    );
    vi.mocked(authCommands.decodeClaims).mockResolvedValue({
      status: "ok",
      data: {
        sub: "user-1",
        email: "test@example.com",
        entitlements: [],
        subscription_status: "trialing",
        trial_end: Math.floor(Date.now() / 1000) + 6 * 24 * 60 * 60,
        has_payment_method: true,
      },
    });

    renderBillingProvider();

    await waitFor(() => {
      expect(
        screen
          .getByTestId("trial-payment-reminder-dialog")
          .getAttribute("data-open"),
      ).toBe("false");
    });
  });
});
