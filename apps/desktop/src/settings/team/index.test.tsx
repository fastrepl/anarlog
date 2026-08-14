import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  billing: {
    isPro: false,
    isReady: true,
    isUpgradingToPro: false,
    upgradeToPro: vi.fn(),
  },
  session: { user: { id: "user-1" } } as { user: { id: string } } | null,
  workspaces: {
    data: [] as Array<{
      workspaceId: string;
      name: string;
      ownerUserId: string;
      role: "owner" | "admin" | "member";
    }>,
    isPending: false,
  },
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session, supabase: {} }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("./mirror", () => ({
  MY_WORKSPACES_QUERY_KEY: "team-workspaces",
  useMyWorkspacesWithMirror: () => mocks.workspaces,
}));

import { SettingsTeam } from "./index";

function renderTeam() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsTeam />
    </QueryClientProvider>,
  );
}

describe("SettingsTeam", () => {
  beforeEach(() => {
    mocks.billing.isPro = false;
    mocks.billing.isReady = true;
    mocks.billing.isUpgradingToPro = false;
    mocks.billing.upgradeToPro.mockClear();
    mocks.session = { user: { id: "user-1" } };
    mocks.workspaces.data = [];
    mocks.workspaces.isPending = false;
  });

  afterEach(cleanup);

  it("offers an upgrade instead of Team controls on the free plan", () => {
    renderTeam();

    expect(screen.getByText("Anarlog Pro required")).toBeTruthy();
    expect(screen.queryByRole("textbox")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Upgrade to Pro" }));

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
  });

  it("shows workspace creation controls on Pro", () => {
    mocks.billing.isPro = true;

    renderTeam();

    expect(screen.getByText("Create a shared workspace")).toBeTruthy();
    expect(screen.getByRole("textbox")).toBeTruthy();
    expect(screen.queryByText("Anarlog Pro required")).toBeNull();
  });

  it("keeps existing workspaces accessible without Pro", () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Existing workspace",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];

    renderTeam();

    expect(screen.getByText("Existing workspace")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Delete workspace" }),
    ).toBeTruthy();
    expect(screen.queryByText("Anarlog Pro required")).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
  });
});
