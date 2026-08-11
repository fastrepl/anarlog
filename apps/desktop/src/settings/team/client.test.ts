import { describe, expect, it, vi } from "vitest";

import {
  createWorkspace,
  getSeatUsage,
  listWorkspaceInvitations,
  listWorkspaceMembers,
  removeMember,
  requireTeamContext,
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
