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
  billingCheckout: {
    buildWebAppUrl: vi.fn(() => Promise.resolve("https://anarlog.so/team")),
    openUrl: vi.fn(() => Promise.resolve()),
    openUrlWithInstruction: vi.fn(
      (_url: string, _kind: string, open: (url: string) => Promise<void>) =>
        open("https://anarlog.so/team"),
    ),
  },
  session: { user: { id: "user-1" } } as { user: { id: string } } | null,
  workspaces: {
    data: [] as Array<{
      workspaceId: string;
      name: string;
      ownerUserId: string;
      shareSlug?: string | null;
      logoDataUrl?: string | null;
      role: "owner" | "admin" | "member";
    }>,
    isPending: false,
  },
  client: {
    access: {
      role: "owner" as const,
      tier: "team" as "free" | "team" | "enterprise",
      capabilities: [
        "team.shared_notes",
        "team.manage_workspace",
        "team.manage_members",
        "team.manage_policies",
        "team.view_usage",
        "team.custom_subdomain",
      ] as string[],
      seatLimit: 1 as number | null,
      usedSeats: 1,
    },
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
    usage: {
      memberCount: 1,
      pendingInvitations: 0,
      enrolledDevices: 0,
      sharesCreated30d: 0,
      shareAccessEvents30d: 0,
      seatLimit: 1 as number | null,
      usedSeats: 1,
      isBilled: true,
    },
    revokeInvitation: vi.fn(() => Promise.resolve()),
    renameWorkspace: vi.fn(() => Promise.resolve()),
    setWorkspaceLogo: vi.fn(() =>
      Promise.resolve({ logoDataUrl: "data:image/jpeg;base64,/9j/4AAQ" }),
    ),
    getWorkspacePolicy: vi.fn(() =>
      Promise.resolve({
        allowedShareScopes: ["restricted", "workspace", "link", "public"],
        defaultShareScope: "restricted",
        retentionDays: null,
        modelTrainingOptOut: true,
        consentNotificationEnabled: true,
        requireSso: false,
      }),
    ),
    setWorkspaceShareSlug: vi.fn(() =>
      Promise.resolve({
        shareSlug: "fastrepl",
        shareBaseUrl: "https://fastrepl.anarlog.so",
      }),
    ),
  },
  invitation: {
    deliverWorkspaceInvitation: vi.fn(() =>
      Promise.resolve({ deliveredBy: "email" as const }),
    ),
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

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.billingCheckout.openUrl },
}));

vi.mock("@anlg/plugin-windows", () => ({
  openUrlWithInstruction: mocks.billingCheckout.openUrlWithInstruction,
}));

vi.mock("~/shared/utils", () => ({
  buildWebAppUrl: mocks.billingCheckout.buildWebAppUrl,
}));

vi.mock("~/env", () => ({
  env: { VITE_ENTERPRISE_API_URL: undefined },
}));

vi.mock("./invitation", () => ({
  deliverWorkspaceInvitation: mocks.invitation.deliverWorkspaceInvitation,
  getTeamSenderName: () => "Owner",
  reportWorkspaceInvitation: vi.fn(),
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
  leaveWorkspace: vi.fn(() => Promise.resolve()),
  listWorkspaceInvitations: () => Promise.resolve(mocks.client.invitations),
  listWorkspaceMembers: () => Promise.resolve(mocks.client.members),
  removeMember: vi.fn(() => Promise.resolve()),
  renameWorkspace: mocks.client.renameWorkspace,
  setWorkspaceLogo: mocks.client.setWorkspaceLogo,
  revokeInvitation: mocks.client.revokeInvitation,
  setMemberRole: vi.fn(() => Promise.resolve()),
  transferOwnership: vi.fn(() => Promise.resolve()),
  getWorkspaceUsageOverview: () => Promise.resolve(mocks.client.usage),
  getWorkspaceAccess: () => Promise.resolve(mocks.client.access),
  getWorkspacePolicy: mocks.client.getWorkspacePolicy,
  setWorkspacePolicy: vi.fn(() => Promise.resolve()),
  setWorkspaceShareSlug: mocks.client.setWorkspaceShareSlug,
  claimWorkspaceDomain: vi.fn(() => Promise.resolve()),
  rotateWorkspaceScimToken: vi.fn(() => Promise.resolve()),
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

function openWorkspace(name: string) {
  fireEvent.click(screen.getByRole("button", { name }));
}

describe("SettingsTeam", () => {
  beforeEach(() => {
    mocks.session = { user: { id: "user-1" } };
    mocks.workspaces.data = [];
    mocks.workspaces.isPending = false;
    mocks.client.members = [];
    mocks.client.invitations = [];
    mocks.client.usage = {
      memberCount: 1,
      pendingInvitations: 0,
      enrolledDevices: 0,
      sharesCreated30d: 0,
      shareAccessEvents30d: 0,
      seatLimit: 1,
      usedSeats: 1,
      isBilled: true,
    };
    mocks.client.access = {
      role: "owner",
      tier: "team",
      capabilities: [
        "team.shared_notes",
        "team.manage_workspace",
        "team.manage_members",
        "team.manage_policies",
        "team.view_usage",
        "team.custom_subdomain",
      ],
      seatLimit: 1,
      usedSeats: 1,
    };
    mocks.client.revokeInvitation.mockClear();
    mocks.client.renameWorkspace.mockClear();
    mocks.client.setWorkspaceLogo.mockClear();
    mocks.client.getWorkspacePolicy.mockClear();
    mocks.client.setWorkspaceShareSlug.mockClear();
    mocks.invitation.deliverWorkspaceInvitation.mockClear();
    mocks.billingCheckout.buildWebAppUrl.mockClear();
    mocks.billingCheckout.openUrl.mockClear();
    mocks.billingCheckout.openUrlWithInstruction.mockClear();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("lets a signed-in account create a Team workspace without personal Pro", () => {
    renderTeam();

    expect(screen.getByText("Create a shared workspace")).toBeTruthy();
    expect(screen.getByRole("textbox")).toBeTruthy();
  });

  it("keeps an unbilled workspace accessible and offers Team checkout", async () => {
    mocks.client.usage.isBilled = false;
    mocks.client.usage.seatLimit = null;
    mocks.client.access.tier = "free";
    mocks.client.access.capabilities = [];
    mocks.client.access.seatLimit = null;
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Existing workspace",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];

    renderTeam();

    expect(
      screen.getByRole("button", { name: "Existing workspace" }),
    ).toBeTruthy();
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(
      screen.queryByRole("textbox", { name: "Workspace name" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Change workspace logo" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Delete workspace" }),
    ).toBeTruthy();
    const checkout = await screen.findByRole("button", {
      name: "Continue to Team checkout",
    });
    await waitFor(() =>
      expect((checkout as HTMLButtonElement).disabled).toBe(false),
    );
    fireEvent.click(checkout);
    await waitFor(() =>
      expect(mocks.billingCheckout.buildWebAppUrl).toHaveBeenCalledWith(
        "/app/team-checkout",
        {
          workspace_id: "00000000-0000-4000-8000-000000000001",
          period: "monthly",
          quantity: "1",
        },
      ),
    );
  });

  it("renames the workspace through the name field", async () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];

    renderTeam();

    const input = await screen.findByRole("textbox", {
      name: "Workspace name",
    });
    fireEvent.change(input, { target: { value: "Fastrepl HQ" } });
    fireEvent.blur(input);

    await waitFor(() =>
      expect(mocks.client.renameWorkspace).toHaveBeenCalledWith(
        expect.anything(),
        "00000000-0000-4000-8000-000000000001",
        "Fastrepl HQ",
      ),
    );
  });

  it("uploads a workspace logo from the identity tile", async () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];
    const jpeg = "data:image/jpeg;base64,/9j/4AAQ";
    const context = {
      drawImage: vi.fn(),
      fillRect: vi.fn(),
      fillStyle: "",
      imageSmoothingQuality: "low",
    };
    vi.stubGlobal("URL", {
      createObjectURL: vi.fn(() => "blob:logo"),
      revokeObjectURL: vi.fn(),
    });
    vi.stubGlobal(
      "Image",
      class {
        naturalHeight = 128;
        naturalWidth = 128;
        onerror: (() => void) | null = null;
        onload: (() => void) | null = null;

        set src(_value: string) {
          queueMicrotask(() => this.onload?.());
        }
      },
    );
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      context as unknown as CanvasRenderingContext2D,
    );
    vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue(jpeg);

    const { container } = renderTeam();
    await screen.findByRole("button", { name: "Change workspace logo" });
    const input =
      container.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input).not.toBeNull();
    if (!input) return;

    fireEvent.change(input, {
      target: {
        files: [new File(["logo"], "logo.png", { type: "image/png" })],
      },
    });

    await waitFor(() =>
      expect(mocks.client.setWorkspaceLogo).toHaveBeenCalledWith(
        expect.anything(),
        "00000000-0000-4000-8000-000000000001",
        jpeg,
      ),
    );
  });

  it("removes a workspace logo from the identity tile", async () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        logoDataUrl: "data:image/jpeg;base64,/9j/4AAQ",
        role: "owner",
      },
    ];

    renderTeam();

    fireEvent.click(
      await screen.findByRole("button", { name: "Remove workspace logo" }),
    );

    await waitFor(() =>
      expect(mocks.client.setWorkspaceLogo).toHaveBeenCalledWith(
        expect.anything(),
        "00000000-0000-4000-8000-000000000001",
        null,
      ),
    );
  });

  it("sets the workspace sharing subdomain", async () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        shareSlug: "fastrepl",
        role: "owner",
      },
    ];

    renderTeam();

    const input = await screen.findByRole("textbox", {
      name: "Workspace subdomain",
    });
    expect((input as HTMLInputElement).value).toBe("fastrepl");
    fireEvent.change(input, { target: { value: "Fastrepl-HQ" } });
    fireEvent.click(screen.getByRole("button", { name: "Save subdomain" }));

    await waitFor(() =>
      expect(mocks.client.setWorkspaceShareSlug).toHaveBeenCalledWith(
        expect.anything(),
        "00000000-0000-4000-8000-000000000001",
        "fastrepl-hq",
      ),
    );
  });

  it("keeps Enterprise policy controls hidden on Team", async () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];

    renderTeam();

    await screen.findByText("Policies");
    expect(screen.queryByText("Require SSO")).toBeNull();
    expect(screen.queryByText("Retention (days)")).toBeNull();
    expect(screen.queryByText("SCIM bearer token")).toBeNull();
  });

  it("shows Enterprise policy controls only with Enterprise capabilities", async () => {
    mocks.client.access.tier = "enterprise";
    mocks.client.access.capabilities = [
      ...mocks.client.access.capabilities,
      "enterprise.sso",
      "enterprise.scim",
      "enterprise.retention",
      "enterprise.audit_logs",
      "enterprise.capture",
    ];
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
    ];

    renderTeam();

    expect(await screen.findByText("Require SSO")).toBeTruthy();
    expect(screen.getByText("Retention (days)")).toBeTruthy();
    expect(screen.getByText("SCIM bearer token")).toBeTruthy();
  });

  it("resends a pending invitation by delivering a fresh invite", async () => {
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
      expect(mocks.invitation.deliverWorkspaceInvitation).toHaveBeenCalledWith({
        context: expect.anything(),
        workspaceId: "00000000-0000-4000-8000-000000000001",
        workspaceName: "Fastrepl",
        email: "teammate@company.com",
        senderName: "Owner",
      }),
    );
  });

  it("switches teams from the tab row", async () => {
    mocks.workspaces.data = [
      {
        workspaceId: "00000000-0000-4000-8000-000000000001",
        name: "Fastrepl",
        ownerUserId: "user-1",
        role: "owner",
      },
      {
        workspaceId: "00000000-0000-4000-8000-000000000002",
        name: "Acme",
        ownerUserId: "client-1",
        role: "member",
      },
    ];

    renderTeam();

    expect(screen.getByRole("button", { name: "Fastrepl" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Acme" })).toBeTruthy();
    expect(
      await screen.findByRole("textbox", { name: "Workspace name" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Delete workspace" }),
    ).toBeTruthy();

    openWorkspace("Acme");

    expect(screen.getByRole("button", { name: "Fastrepl" })).toBeTruthy();
    expect(
      screen.queryByRole("textbox", { name: "Workspace name" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Leave workspace" }),
    ).toBeTruthy();
  });
});
