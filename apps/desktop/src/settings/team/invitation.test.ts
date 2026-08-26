import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createWorkspaceInvitation: vi.fn(),
  resendWorkspaceInvitation: vi.fn(),
  revokeInvitation: vi.fn(),
  sendWorkspaceInvitationEmail: vi.fn(),
  writeClipboardText: vi.fn(),
  isTauri: vi.fn(() => false),
  env: {
    VITE_API_URL: "https://api.anarlog.so",
    VITE_APP_URL: "https://anarlog.so",
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: mocks.isTauri,
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.writeClipboardText,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("~/env", () => ({
  env: mocks.env,
}));

vi.mock("./client", () => ({
  TeamError: class TeamError extends Error {
    constructor(message = "Workspace request failed") {
      super(message);
      this.name = "TeamError";
    }
  },
  createWorkspaceInvitation: mocks.createWorkspaceInvitation,
  resendWorkspaceInvitation: mocks.resendWorkspaceInvitation,
  revokeInvitation: mocks.revokeInvitation,
  sendWorkspaceInvitationEmail: mocks.sendWorkspaceInvitationEmail,
}));

import { TeamError, type TeamContext } from "./client";
import { deliverWorkspaceInvitation } from "./invitation";

const WORKSPACE_ID = "11111111-1111-4111-8111-111111111111";
const INVITATION_ID = "22222222-2222-4222-8222-222222222222";
const ROTATED_INVITATION_ID = "33333333-3333-4333-8333-333333333333";
const TOKEN = "A".repeat(43);

function context(): TeamContext {
  return {
    supabase: {} as TeamContext["supabase"],
    session: {
      access_token: "token",
      user: { id: "user-1" },
    } as TeamContext["session"],
  };
}

describe("deliverWorkspaceInvitation", () => {
  beforeEach(() => {
    mocks.createWorkspaceInvitation.mockReset();
    mocks.resendWorkspaceInvitation.mockReset();
    mocks.revokeInvitation.mockReset();
    mocks.sendWorkspaceInvitationEmail.mockReset();
    mocks.writeClipboardText.mockReset();
    mocks.isTauri.mockReturnValue(false);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn(() => Promise.resolve()) },
    });
  });

  it("sends email for a newly created invitation", async () => {
    mocks.createWorkspaceInvitation.mockResolvedValue({
      invitationId: INVITATION_ID,
      inviteToken: TOKEN,
      expiresAt: "2026-09-01T00:00:00Z",
      wasCreated: true,
    });
    mocks.sendWorkspaceInvitationEmail.mockResolvedValue(undefined);

    await expect(
      deliverWorkspaceInvitation({
        context: context(),
        workspaceId: WORKSPACE_ID,
        workspaceName: "Fastrepl",
        email: "artem@fastrepl.com",
        senderName: "John",
      }),
    ).resolves.toEqual({ deliveredBy: "email" });

    expect(mocks.sendWorkspaceInvitationEmail).toHaveBeenCalledWith(
      expect.objectContaining({
        invitationId: INVITATION_ID,
        inviteToken: TOKEN,
        workspaceName: "Fastrepl",
      }),
    );
    expect(mocks.revokeInvitation).not.toHaveBeenCalled();
  });

  it("rotates a pending invitation so a resend has a fresh token", async () => {
    mocks.createWorkspaceInvitation.mockResolvedValue({
      invitationId: INVITATION_ID,
      inviteToken: null,
      expiresAt: "2026-09-01T00:00:00Z",
      wasCreated: false,
    });
    mocks.resendWorkspaceInvitation.mockResolvedValue({
      invitationId: ROTATED_INVITATION_ID,
      inviteToken: TOKEN,
      expiresAt: "2026-09-01T00:00:00Z",
      wasCreated: true,
    });
    mocks.sendWorkspaceInvitationEmail.mockResolvedValue(undefined);

    await expect(
      deliverWorkspaceInvitation({
        context: context(),
        workspaceId: WORKSPACE_ID,
        workspaceName: "Fastrepl",
        email: "artem@fastrepl.com",
        senderName: "John",
      }),
    ).resolves.toEqual({ deliveredBy: "email" });

    expect(mocks.resendWorkspaceInvitation).toHaveBeenCalledWith(
      expect.anything(),
      INVITATION_ID,
    );
    expect(mocks.createWorkspaceInvitation).toHaveBeenCalledTimes(1);
    expect(mocks.revokeInvitation).not.toHaveBeenCalled();
    expect(mocks.sendWorkspaceInvitationEmail).toHaveBeenCalledWith(
      expect.objectContaining({
        invitationId: ROTATED_INVITATION_ID,
        inviteToken: TOKEN,
      }),
    );
  });

  it("keeps the pending invitation when atomic rotation fails", async () => {
    mocks.createWorkspaceInvitation.mockResolvedValue({
      invitationId: INVITATION_ID,
      inviteToken: null,
      expiresAt: "2026-09-01T00:00:00Z",
      wasCreated: false,
    });
    mocks.resendWorkspaceInvitation.mockRejectedValue(new TeamError());

    await expect(
      deliverWorkspaceInvitation({
        context: context(),
        workspaceId: WORKSPACE_ID,
        workspaceName: "Fastrepl",
        email: "artem@fastrepl.com",
        senderName: "John",
      }),
    ).rejects.toThrow(TeamError);

    expect(mocks.revokeInvitation).not.toHaveBeenCalled();
    expect(mocks.sendWorkspaceInvitationEmail).not.toHaveBeenCalled();
  });

  it("copies the invite link when email delivery is unavailable", async () => {
    mocks.createWorkspaceInvitation.mockResolvedValue({
      invitationId: INVITATION_ID,
      inviteToken: TOKEN,
      expiresAt: "2026-09-01T00:00:00Z",
      wasCreated: true,
    });
    mocks.sendWorkspaceInvitationEmail.mockRejectedValue(new TeamError());
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    await expect(
      deliverWorkspaceInvitation({
        context: context(),
        workspaceId: WORKSPACE_ID,
        workspaceName: "Fastrepl",
        email: "artem@fastrepl.com",
        senderName: "John",
      }),
    ).resolves.toEqual({ deliveredBy: "clipboard" });

    expect(writeText).toHaveBeenCalledWith(
      `https://anarlog.so/team/invite/${INVITATION_ID}/#token=${TOKEN}`,
    );
  });
});
