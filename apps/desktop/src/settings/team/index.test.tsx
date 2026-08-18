import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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
  client: {
    members: [] as Array<{
      userId: string;
      email: string;
      role: "owner" | "admin" | "member";
    }>,
    invitations: [] as Array<{
      invitationId: string;
      email: string;
      expiresAt: string;
    }>,
    inviteMember: vi.fn(() => Promise.resolve()),
    revokeInvitation: vi.fn(() => Promise.resolve()),
    renameWorkspace: vi.fn(() => Promise.resolve()),
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

vi.mock("./client", () => ({
  requireTeamContext: (auth: unknown) => auth,
  createWorkspace: vi.fn(() => Promise.resolve({ workspaceId: "ws" })),
  deleteWorkspace: vi.fn(() => Promise.resolve()),
  getSeatUsage: () =>
    Promise.resolve({ seatLimit: null, usedSeats: 1, isBilled: false }),
  inviteMember: mocks.client.inviteMember,
  leaveWorkspace: vi.fn(() => Promise.resolve()),
  listWorkspaceInvitations: () => Promise.resolve(mocks.client.invitations),
  listWorkspaceMembers: () => Promise.resolve(mocks.client.members),
  removeMember: vi.fn(() => Promise.resolve()),
  renameWorkspace: mocks.client.renameWorkspace,
  revokeInvitation: mocks.client.revokeInvitation,
  setMemberRole: vi.fn(() => Promise.resolve()),
  transferOwnership: vi.fn(() => Promise.resolve()),
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
    mocks.client.members = [];
    mocks.client.invitations = [];
    mocks.client.inviteMember.mockClear();
    mocks.client.revokeInvitation.mockClear();
    mocks.client.renameWorkspace.mockClear();
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
    expect(screen.getByRole("combobox")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Delete workspace" }),
    ).toBeTruthy();
    expect(screen.queryByText("Anarlog Pro required")).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("renames the workspace through the edit button", async () => {
    mocks.billing.isPro = true;
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];

    renderTeam();

    fireEvent.click(screen.getByRole("button", { name: "Rename workspace" }));

    const input = screen.getByRole("textbox", { name: "Workspace name" });
    fireEvent.change(input, { target: { value: "Fastrepl HQ" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() =>
      expect(mocks.client.renameWorkspace).toHaveBeenCalledWith(
        expect.anything(),
        "00000000-0000-4000-8000-000000000001",
        "Fastrepl HQ",
      ),
    );
    expect(
      screen.queryByRole("textbox", { name: "Workspace name" }),
    ).toBeNull();
  });

  it("resends a pending invitation by revoking and reinviting", async () => {
    mocks.billing.isPro = true;
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];
    mocks.client.invitations = [
      {
        invitationId: "00000000-0000-4000-8000-00000000000a",
        email: "teammate@company.com",
        expiresAt: "2026-09-17T00:00:00Z",
      },
    ];

    renderTeam();

    fireEvent.click(
      await screen.findByRole("button", { name: "Resend invitation" }),
    );

    await waitFor(() =>
      expect(mocks.client.inviteMember).toHaveBeenCalledWith(
        expect.anything(),
        "00000000-0000-4000-8000-000000000001",
        "teammate@company.com",
      ),
    );
    expect(mocks.client.revokeInvitation).toHaveBeenCalledWith(
      expect.anything(),
      "00000000-0000-4000-8000-00000000000a",
    );
  });
});
