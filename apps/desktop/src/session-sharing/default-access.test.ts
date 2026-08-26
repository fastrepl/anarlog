import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  setSessionShareScope: vi.fn(),
  deliverSessionShareInvitations: vi.fn(),
  getSessionShareSenderName: vi.fn(() => "John Jeong"),
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("./client", () => ({
  setSessionShareScope: mocks.setSessionShareScope,
}));

vi.mock("./invitation-management", () => ({
  isInviteEmail: (value: string) =>
    value.trim().length <= 320 &&
    /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim()),
  deliverSessionShareInvitations: mocks.deliverSessionShareInvitations,
  getSessionShareSenderName: mocks.getSessionShareSenderName,
}));

import {
  applyDefaultMeetingShareAccess,
  defaultGeneralAccessTarget,
  loadMeetingShareInviteEmails,
  normalizeDefaultMeetingShareAccess,
} from "./default-access";

const SHARE_ID = "33333333-3333-4333-8333-333333333333";
const WORKSPACE_ID = "22222222-2222-4222-8222-222222222222";
const SESSION_ID = "session-1";

function context() {
  return {
    supabase: {},
    session: {
      user: { id: "user-1", email: "owner@example.com" },
    },
  } as never;
}

describe("normalizeDefaultMeetingShareAccess", () => {
  it("keeps known values and falls back to only me", () => {
    expect(normalizeDefaultMeetingShareAccess("me")).toBe("me");
    expect(normalizeDefaultMeetingShareAccess("participants")).toBe(
      "participants",
    );
    expect(normalizeDefaultMeetingShareAccess("workspace")).toBe("workspace");
    expect(normalizeDefaultMeetingShareAccess("link")).toBe("me");
    expect(normalizeDefaultMeetingShareAccess(undefined)).toBe("me");
  });
});

describe("defaultGeneralAccessTarget", () => {
  it("uses the first workspace when the default is workspace access", () => {
    expect(
      defaultGeneralAccessTarget("workspace", [
        { id: WORKSPACE_ID, name: "Fastrepl" },
      ]),
    ).toBe(`workspace:${WORKSPACE_ID}`);
  });

  it("stays restricted without a workspace or for private defaults", () => {
    expect(defaultGeneralAccessTarget("workspace", [])).toBe("restricted");
    expect(
      defaultGeneralAccessTarget("participants", [
        { id: WORKSPACE_ID, name: "Fastrepl" },
      ]),
    ).toBe("restricted");
    expect(defaultGeneralAccessTarget("me", [])).toBe("restricted");
  });
});

describe("loadMeetingShareInviteEmails", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("keeps unique attendee emails and skips the owner", async () => {
    mocks.execute.mockResolvedValue([
      { email: "sungbin@e.com" },
      { email: " owner@example.com " },
      { email: "yujong@e.com" },
      { email: "sungbin@e.com" },
      { email: "not-an-email" },
      { email: "" },
    ]);

    await expect(
      loadMeetingShareInviteEmails({
        sessionId: SESSION_ID,
        ownerEmail: "owner@example.com",
      }),
    ).resolves.toEqual(["sungbin@e.com", "yujong@e.com"]);
    expect(mocks.execute).toHaveBeenCalledWith(
      expect.stringContaining("FROM session_participants AS participant"),
      [SESSION_ID],
    );
  });
});

describe("applyDefaultMeetingShareAccess", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.setSessionShareScope.mockResolvedValue({});
    mocks.deliverSessionShareInvitations.mockResolvedValue([]);
    mocks.execute.mockResolvedValue([{ email: "sungbin@e.com" }]);
  });

  const base = {
    wasCreated: true,
    actionType: "copy-link" as const,
    workspaces: [{ id: WORKSPACE_ID, name: "Fastrepl" }],
    context: context(),
    shareId: SHARE_ID,
    sessionId: SESSION_ID,
    noteTitle: "Planning",
    signal: new AbortController().signal,
    requireActive: vi.fn(),
  };

  it("does not change an existing share or an explicit scope action", async () => {
    await applyDefaultMeetingShareAccess({
      ...base,
      wasCreated: false,
      access: "workspace",
    });
    await applyDefaultMeetingShareAccess({
      ...base,
      actionType: "scope",
      access: "workspace",
    });

    expect(mocks.setSessionShareScope).not.toHaveBeenCalled();
    expect(mocks.deliverSessionShareInvitations).not.toHaveBeenCalled();
  });

  it("opens the share to the workspace", async () => {
    await applyDefaultMeetingShareAccess({
      ...base,
      access: "workspace",
    });

    expect(mocks.setSessionShareScope).toHaveBeenCalledWith(base.context, {
      shareId: SHARE_ID,
      scope: "workspace",
      workspaceId: WORKSPACE_ID,
    });
    expect(base.requireActive).toHaveBeenCalled();
  });

  it("invites meeting participants when copying a link", async () => {
    await applyDefaultMeetingShareAccess({
      ...base,
      access: "participants",
    });

    expect(mocks.deliverSessionShareInvitations).toHaveBeenCalledWith(
      expect.objectContaining({
        shareId: SHARE_ID,
        emails: ["sungbin@e.com"],
        capability: "viewer",
        noteTitle: "Planning",
      }),
    );
  });

  it("leaves invites to the share action when the user is already inviting", async () => {
    await applyDefaultMeetingShareAccess({
      ...base,
      actionType: "invite",
      access: "participants",
    });

    expect(mocks.deliverSessionShareInvitations).not.toHaveBeenCalled();
  });

  it("skips workspace access when no workspace is available", async () => {
    await applyDefaultMeetingShareAccess({
      ...base,
      access: "workspace",
      workspaces: [],
    });

    expect(mocks.setSessionShareScope).not.toHaveBeenCalled();
  });
});
