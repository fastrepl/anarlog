import { describe, expect, it, vi } from "vitest";

import {
  checkWorkspaceShareSlugAvailability,
  createWorkspace,
  createWorkspaceInvitation,
  getSeatUsage,
  getWorkspaceAccess,
  getWorkspacePolicy,
  intersectAllowedShareScopes,
  listWorkspaceInvitations,
  listWorkspaceMembers,
  removeMember,
  requireTeamContext,
  resendWorkspaceInvitation,
  sendWorkspaceInvitationEmail,
  setWorkspaceLogo,
  setWorkspaceShareSlug,
  TeamError,
  type TeamContext,
} from "./client";

const WORKSPACE_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";

function context(data: unknown, error: { message: string } | null = null) {
  const rpc = vi.fn(() => ({
    setHeader: () => Promise.resolve({ data, error }),
  }));
  return {
    context: {
      supabase: { rpc },
      session: { access_token: "token", user: { id: USER_ID } },
    } as unknown as TeamContext,
    rpc,
  };
}

describe("requireTeamContext", () => {
  it("refuses anonymous and signed-out callers", () => {
    expect(() => requireTeamContext({})).toThrow(TeamError);
    expect(() =>
      requireTeamContext({
        supabase: {} as never,
        session: { user: { is_anonymous: true } } as never,
      }),
    ).toThrow(TeamError);
  });
});

describe("workspace reads", () => {
  it("parses workspace-scoped capabilities and ignores future additions", async () => {
    const { context: ctx } = context([
      {
        workspace_role: "admin",
        workspace_tier: "enterprise",
        capabilities: [
          "team.manage_members",
          "enterprise.capture",
          "future.capability",
        ],
        seat_limit: 12,
        used_seats: 4,
      },
    ]);

    await expect(getWorkspaceAccess(ctx, WORKSPACE_ID)).resolves.toEqual({
      role: "admin",
      tier: "enterprise",
      capabilities: ["team.manage_members", "enterprise.capture"],
      seatLimit: 12,
      usedSeats: 4,
    });
  });

  it("keeps only active members and normalizes their role", async () => {
    const { context: ctx } = context([
      {
        user_id: USER_ID,
        user_email: "a@example.com",
        role: "owner",
        deleted_at: null,
      },
      {
        user_id: WORKSPACE_ID,
        user_email: "gone@example.com",
        role: "member",
        deleted_at: "2026-08-11T00:00:00Z",
      },
    ]);

    await expect(listWorkspaceMembers(ctx, WORKSPACE_ID)).resolves.toEqual([
      { userId: USER_ID, email: "a@example.com", role: "owner" },
    ]);
  });

  it("sets the canonical workspace sharing subdomain", async () => {
    const { context: ctx, rpc } = context([
      {
        workspace_id: WORKSPACE_ID,
        workspace_share_slug: "fastrepl",
        share_base_url: "https://fastrepl.anarlog.so",
      },
    ]);

    await expect(
      setWorkspaceShareSlug(ctx, WORKSPACE_ID, "Fastrepl"),
    ).resolves.toEqual({
      shareSlug: "fastrepl",
      shareBaseUrl: "https://fastrepl.anarlog.so",
    });
    expect(rpc).toHaveBeenCalledWith("set_workspace_share_slug", {
      p_workspace_id: WORKSPACE_ID,
      p_slug: "Fastrepl",
    });
  });

  it("checks workspace sharing subdomain availability", async () => {
    const { context: ctx, rpc } = context("available");

    await expect(
      checkWorkspaceShareSlugAvailability(ctx, WORKSPACE_ID, "fastrepl"),
    ).resolves.toBe("available");
    expect(rpc).toHaveBeenCalledWith(
      "check_workspace_share_slug_availability",
      {
        p_workspace_id: WORKSPACE_ID,
        p_slug: "fastrepl",
      },
    );
  });

  it("rejects an unknown workspace sharing subdomain availability", async () => {
    const { context: ctx } = context("unknown");

    await expect(
      checkWorkspaceShareSlugAvailability(ctx, WORKSPACE_ID, "fastrepl"),
    ).rejects.toThrow(TeamError);
  });

  it("sets and clears the workspace logo", async () => {
    const jpeg = "data:image/jpeg;base64,/9j/4AAQ";
    const { context: ctx, rpc } = context([
      {
        workspace_id: WORKSPACE_ID,
        workspace_logo_data: jpeg,
      },
    ]);

    await expect(setWorkspaceLogo(ctx, WORKSPACE_ID, jpeg)).resolves.toEqual({
      logoDataUrl: jpeg,
    });
    expect(rpc).toHaveBeenCalledWith("set_workspace_logo", {
      p_workspace_id: WORKSPACE_ID,
      p_logo_data: jpeg,
    });
  });

  it("treats a cleared logo as null rather than a missing row", async () => {
    const { context: ctx } = context([
      {
        workspace_id: WORKSPACE_ID,
        workspace_logo_data: null,
      },
    ]);

    await expect(setWorkspaceLogo(ctx, WORKSPACE_ID, null)).resolves.toEqual({
      logoDataUrl: null,
    });
  });

  it("hides invitations that were already accepted or revoked", async () => {
    const { context: ctx } = context([
      {
        invitation_id: WORKSPACE_ID,
        invitee_email: "pending@example.com",
        expires_at: "2026-09-01T00:00:00Z",
        accepted_at: null,
        revoked_at: null,
      },
      {
        invitation_id: USER_ID,
        invitee_email: "joined@example.com",
        expires_at: "2026-09-01T00:00:00Z",
        accepted_at: "2026-08-11T00:00:00Z",
        revoked_at: null,
      },
    ]);

    const invitations = await listWorkspaceInvitations(ctx, WORKSPACE_ID);
    expect(invitations.map((invitation) => invitation.email)).toEqual([
      "pending@example.com",
    ]);
  });

  it("reads an unlimited workspace as having no seat cap", async () => {
    const { context: ctx } = context([
      { seat_limit: null, used_seats: 3, is_billed: false },
    ]);

    await expect(getSeatUsage(ctx, WORKSPACE_ID)).resolves.toEqual({
      seatLimit: null,
      usedSeats: 3,
      isBilled: false,
    });
  });

  it("intersects org share policies so clients hide disallowed scopes", async () => {
    const { context: ctx } = context([
      {
        allowed_share_scopes: ["restricted", "workspace"],
        default_share_scope: "restricted",
        retention_days: 30,
        model_training_opt_out: true,
        consent_notification_enabled: true,
        require_sso: false,
      },
    ]);

    const policy = await getWorkspacePolicy(ctx, WORKSPACE_ID);
    expect(
      intersectAllowedShareScopes([
        policy,
        {
          allowedShareScopes: ["restricted", "workspace", "link", "public"],
        },
      ]),
    ).toEqual(["restricted", "workspace"]);
  });
});

describe("failure handling", () => {
  it("surfaces the server's reason so managers see why an action failed", async () => {
    const { context: ctx } = context(null, {
      message: "workspace seat limit reached",
    });

    await expect(createWorkspace(ctx, "Acme")).rejects.toThrow(
      "workspace seat limit reached",
    );
  });

  it("rejects malformed identifiers before reaching the network", async () => {
    const { context: ctx, rpc } = context([]);

    await expect(removeMember(ctx, "not-a-uuid", USER_ID)).rejects.toThrow(
      TeamError,
    );
    expect(rpc).not.toHaveBeenCalled();
  });

  it("treats a missing row as a failure rather than returning undefined", async () => {
    const { context: ctx } = context([]);

    await expect(getSeatUsage(ctx, WORKSPACE_ID)).rejects.toThrow(TeamError);
  });
});

describe("invitations", () => {
  const inviteToken = "A".repeat(43);

  it("returns the invite token so the client can send the email", async () => {
    const { context: ctx, rpc } = context([
      {
        invitation_id: WORKSPACE_ID,
        invite_token: inviteToken,
        invitation_expires_at: "2026-09-01T00:00:00Z",
        was_created: true,
      },
    ]);

    await expect(
      createWorkspaceInvitation(ctx, WORKSPACE_ID, " Person@Example.com "),
    ).resolves.toEqual({
      invitationId: WORKSPACE_ID,
      inviteToken,
      expiresAt: "2026-09-01T00:00:00Z",
      wasCreated: true,
    });
    expect(rpc).toHaveBeenCalledWith("create_workspace_invitation", {
      p_workspace_id: WORKSPACE_ID,
      p_invitee_email: "person@example.com",
    });
  });

  it("returns a fresh token from the atomic resend RPC", async () => {
    const { context: ctx, rpc } = context([
      {
        invitation_id: USER_ID,
        invite_token: inviteToken,
        invitation_expires_at: "2026-09-01T00:00:00Z",
      },
    ]);

    await expect(resendWorkspaceInvitation(ctx, WORKSPACE_ID)).resolves.toEqual(
      {
        invitationId: USER_ID,
        inviteToken,
        expiresAt: "2026-09-01T00:00:00Z",
        wasCreated: true,
      },
    );
    expect(rpc).toHaveBeenCalledWith("resend_workspace_invitation", {
      p_invitation_id: WORKSPACE_ID,
    });
  });

  it("sends an invitation email through the authenticated API", async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 204 }));

    await expect(
      sendWorkspaceInvitationEmail({
        apiBaseUrl: "https://api.anarlog.so",
        accessToken: "token",
        workspaceId: WORKSPACE_ID,
        invitationId: USER_ID,
        inviteToken,
        workspaceName: "Fastrepl",
        senderName: "Owner",
        fetcher,
      }),
    ).resolves.toBeUndefined();

    const [url, init] = fetcher.mock.calls[0]!;
    expect(url.toString()).toBe(
      `https://api.anarlog.so/workspaces/invitations/${USER_ID}/email`,
    );
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toBe("Bearer token");
    expect(init.body).toBe(
      JSON.stringify({
        workspaceId: WORKSPACE_ID,
        inviteToken,
        workspaceName: "Fastrepl",
        fromName: "Owner",
      }),
    );
  });
});
