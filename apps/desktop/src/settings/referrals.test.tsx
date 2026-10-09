import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: {
    user: { id: "user-a", is_anonymous: false },
    access_token: "token-a",
  } as
    | { user: { id: string; is_anonymous: boolean }; access_token: string }
    | null
    | undefined,
  response: vi.fn(),
  clipboard: "",
  signIn: vi.fn(async () => {}),
  success: vi.fn(),
  error: vi.fn(),
}));
vi.mock("~/auth/auth-context", () => ({
  useAuth: () => ({
    session: mocks.session,
    signIn: mocks.signIn,
    supabase: {
      rpc: () => ({
        setHeader: (_name: string, token: string) => ({
          abortSignal: () => mocks.response(token),
        }),
      }),
    },
  }),
}));
vi.mock("~/env", () => ({ env: { VITE_APP_URL: "https://anarlog.so" } }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: async (value: string) => {
    mocks.clipboard = value;
  },
}));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { success: mocks.success, error: mocks.error },
}));

import { SettingsReferrals } from "./referrals";

const code = "0123456789abcdef01234567";
const summary = (overrides = {}) => ({
  enabled: true,
  eligible: true,
  code,
  accepted: 2,
  remaining: 1,
  months_earned: 1,
  invites: [
    {
      id: "a",
      slot: 1,
      policy: "trial_month",
      accepted_at: "2026-10-05T00:00:00Z",
      extended_until: null,
      legacy_amount_cents: 1400,
      status: "accepted",
    },
    {
      id: "b",
      slot: 2,
      policy: "trial_month",
      accepted_at: "2026-10-05T00:00:00Z",
      extended_until: "2026-12-05T00:00:00Z",
      legacy_amount_cents: 1400,
      status: "applied",
    },
  ],
  ...overrides,
});
const clients: QueryClient[] = [];
function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, retryDelay: 0 } },
  });
  clients.push(client);
  const view = () => (
    <QueryClientProvider client={client}>
      <SettingsReferrals />
    </QueryClientProvider>
  );
  const result = render(view());
  return { ...result, update: () => result.rerender(view()) };
}

afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
});
beforeEach(() => {
  vi.clearAllMocks();
  mocks.session = {
    user: { id: "user-a", is_anonymous: false },
    access_token: "token-a",
  };
  mocks.clipboard = "";
  mocks.response.mockReset();
  mocks.response.mockResolvedValue({
    data: summary(),
    error: null,
  });
});

describe("Referral invites", () => {
  it("copies one personal link while showing acceptance separately from applied rewards", async () => {
    setup();
    const copy = await screen.findByRole("button", { name: "Copy link" });
    expect(
      screen.getByText("Accepted · Waiting for trial to start"),
    ).toBeTruthy();
    expect(
      screen.getByText("Reward applied · 1 free month added"),
    ).toBeTruthy();
    const rows = screen.getAllByRole("listitem");
    expect(within(rows[0]).queryByRole("button")).toBeNull();
    expect(within(rows[1]).queryByRole("button")).toBeNull();
    fireEvent.click(copy);
    await screen.findByRole("button", { name: "Copied" });
    expect(mocks.clipboard).toBe(`https://anarlog.so/invite/${code}`);
  });

  it("shows a retryable server error instead of fabricated or ineligible invites", async () => {
    mocks.response.mockResolvedValue({
      data: null,
      error: { message: "RPC unavailable" },
    });
    setup();
    await screen.findByRole("alert");
    expect(screen.queryByRole("listitem")).toBeNull();
    mocks.response.mockResolvedValue({ data: summary(), error: null });
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByRole("button", { name: "Copy link" });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("offers billing when the server returns no eligible invites", async () => {
    mocks.response.mockResolvedValue({
      data: summary({ eligible: false, code: null, invites: [] }),
      error: null,
    });
    setup();
    await screen.findByRole("button", { name: "View billing" });
    expect(screen.queryByRole("button", { name: "Copy link" })).toBeNull();
  });

  it("does not show the previous account's links while another account loads or signs out", async () => {
    mocks.response.mockImplementation((token: string) =>
      token === "Bearer token-a"
        ? Promise.resolve({ data: summary(), error: null })
        : new Promise(() => {}),
    );
    const view = setup();
    await screen.findByRole("button", { name: "Copy link" });
    mocks.session = {
      user: { id: "user-b", is_anonymous: false },
      access_token: "token-b",
    };
    view.update();
    expect(screen.queryByRole("button", { name: "Copy link" })).toBeNull();
    expect(screen.getByRole("status").textContent).toContain(
      "Loading your invites",
    );
    mocks.session = null;
    view.update();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
    expect(screen.queryByRole("listitem")).toBeNull();
  });
});
