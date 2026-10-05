import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  flushDatabaseWrites: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWrites: mocks.flushDatabaseWrites,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: vi.fn(),
}));

import { loadSessionShareSource } from "./source";

import { DEFAULT_USER_ID } from "~/shared/utils";

const ACCOUNT_ID = "account-1";

function sourceRow(
  overrides: Partial<{
    id: string;
    document_id: string | null;
    workspace_id: string;
    owner_user_id: string | null;
    title: string;
    created_at: string;
    started_at: string;
    participants_json: string;
    body: string;
    body_format: string;
    personal_workspace_available: number | boolean;
    assigned_workspace_kind: string | null;
    assigned_workspace_deleted_at: string | null;
    assigned_workspace_role: string | null;
    binding_json: string | null;
    library_account_id: string | null;
  }> = {},
) {
  return {
    id: "session-1",
    document_id: "summary-1",
    workspace_id: ACCOUNT_ID,
    owner_user_id: ACCOUNT_ID,
    title: "Planning notes",
    created_at: "2026-08-06T00:30:00.000Z",
    started_at: "2026-08-06T01:30:00.000Z",
    participants_json: JSON.stringify([
      { name: "John Jeong" },
      { name: "Sungbin Jo" },
    ]),
    body: JSON.stringify({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Current content" }],
        },
      ],
    }),
    body_format: "prosemirror_json",
    personal_workspace_available: 1,
    assigned_workspace_kind: "personal",
    assigned_workspace_deleted_at: null,
    assigned_workspace_role: "owner",
    binding_json: null,
    ...overrides,
  };
}

describe("loadSessionShareSource", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shares personal library notes through the active account", async () => {
    mocks.execute.mockResolvedValueOnce([
      sourceRow({
        workspace_id: "library-a",
        library_account_id: ACCOUNT_ID,
        personal_workspace_available: 1,
      }),
    ]);
    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toMatchObject({ workspaceId: ACCOUNT_ID });
  });

  it("rejects a library connected to another account even when the personal workspace exists", async () => {
    mocks.execute.mockResolvedValueOnce([
      sourceRow({
        workspace_id: "library-a",
        library_account_id: "other-account",
        personal_workspace_available: 1,
      }),
    ]);
    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).rejects.toThrow();
  });

  it("loads the first summary instead of the raw memo", async () => {
    mocks.execute.mockResolvedValue([sourceRow()]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toEqual({
      sessionId: "session-1",
      documentId: "summary-1",
      workspaceId: ACCOUNT_ID,
      title: "Planning notes",
      meetingAt: "2026-08-06T01:30:00.000Z",
      participants: ["John Jeong", "Sungbin Jo"],
      body: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Current content" }],
          },
        ],
      },
      rawBody: sourceRow().body,
      bodyFormat: "prosemirror_json",
    });

    const [, params] = mocks.execute.mock.calls[0]!;
    expect(params).toEqual([
      ACCOUNT_ID,
      ACCOUNT_ID,
      ACCOUNT_ID,
      ACCOUNT_ID,
      "session-1",
    ]);
    expect(mocks.flushDatabaseWrites).toHaveBeenCalledWith([
      "session:session-1",
    ]);
    expect(mocks.flushDatabaseWrites.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.execute.mock.invocationCallOrder[0]!,
    );
  });

  it("normalizes preview metadata", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        created_at: "2026-08-06T00:30:00.000Z",
        started_at: "not-a-timestamp",
        participants_json: JSON.stringify([
          { name: "A".repeat(101) },
          { name: "John Jeong" },
        ]),
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toMatchObject({
      meetingAt: "2026-08-06T00:30:00.000Z",
      participants: ["John Jeong"],
    });
  });

  it("uses the bound personal workspace while its local projection is missing", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        personal_workspace_available: 0,
        assigned_workspace_kind: null,
        assigned_workspace_role: null,
        binding_json: JSON.stringify({
          workspace_id: ACCOUNT_ID,
          account_user_id: ACCOUNT_ID,
        }),
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toMatchObject({ workspaceId: ACCOUNT_ID });
  });

  it("rejects an unprojected personal workspace without an account binding", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        personal_workspace_available: 0,
        assigned_workspace_kind: null,
        assigned_workspace_role: null,
        binding_json: null,
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).rejects.toThrow("personal workspace is unavailable");
  });

  it.each(["owner", "admin"])(
    "allows an active shared-workspace %s to share the note",
    async (role) => {
      mocks.execute.mockResolvedValue([
        sourceRow({
          workspace_id: "workspace-shared",
          assigned_workspace_kind: "shared",
          assigned_workspace_role: role,
        }),
      ]);

      await expect(
        loadSessionShareSource("session-1", ACCOUNT_ID),
      ).resolves.toMatchObject({ workspaceId: "workspace-shared" });
    },
  );

  it("allows a member to share their own note from a shared workspace", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        workspace_id: "workspace-shared",
        owner_user_id: ACCOUNT_ID,
        assigned_workspace_kind: "shared",
        assigned_workspace_role: "member",
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toMatchObject({ workspaceId: "workspace-shared" });
  });

  it("rejects a member sharing another user's note from a shared workspace", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        workspace_id: "workspace-shared",
        owner_user_id: "other-account",
        assigned_workspace_kind: "shared",
        assigned_workspace_role: "member",
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).rejects.toThrow("You can no longer share notes from this workspace");
  });

  it("fails closed when a known shared-workspace membership is lost", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        workspace_id: "workspace-shared",
        assigned_workspace_kind: "shared",
        assigned_workspace_role: null,
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).rejects.toThrow("no longer share");
  });

  it("fails closed instead of treating a deleted shared workspace as legacy", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        workspace_id: "workspace-shared",
        assigned_workspace_kind: "shared",
        assigned_workspace_deleted_at: "2026-07-17T00:00:00Z",
        assigned_workspace_role: "owner",
        binding_json: JSON.stringify({
          workspace_id: "workspace-shared",
          account_user_id: ACCOUNT_ID,
        }),
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).rejects.toThrow("no longer share");
  });

  it("falls back to the projected personal workspace for a legacy binding", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        workspace_id: "legacy-local-workspace",
        assigned_workspace_kind: null,
        assigned_workspace_role: null,
        binding_json: JSON.stringify({
          workspace_id: "legacy-local-workspace",
          account_user_id: ACCOUNT_ID,
        }),
      }),
    ]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toMatchObject({ workspaceId: ACCOUNT_ID });
  });

  it.each(["", DEFAULT_USER_ID])(
    "falls back to the projected personal workspace for a %s session binding",
    async (workspaceId) => {
      mocks.execute.mockResolvedValue([
        sourceRow({
          workspace_id: workspaceId,
          assigned_workspace_kind: null,
          assigned_workspace_role: null,
        }),
      ]);

      await expect(
        loadSessionShareSource("session-1", ACCOUNT_ID),
      ).resolves.toMatchObject({ workspaceId: ACCOUNT_ID });
    },
  );

  it("refuses to share without a generated summary", async () => {
    for (const row of [
      sourceRow({ document_id: null, body: "" }),
      sourceRow({ body: "" }),
    ]) {
      mocks.execute.mockResolvedValueOnce([row]);
      await expect(
        loadSessionShareSource("session-1", ACCOUNT_ID),
      ).rejects.toThrow("Generate a summary before sharing this note");
    }
  });

  it("converts imported Markdown to ProseMirror JSON", async () => {
    mocks.execute.mockResolvedValue([
      sourceRow({
        body: "# Agenda\n\nDiscuss launch",
        body_format: "markdown",
      }),
    ]);

    const source = await loadSessionShareSource("session-1", ACCOUNT_ID);
    expect(source.body).toMatchObject({
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 1 } },
        { type: "paragraph" },
      ],
    });
  });

  it.each([
    "{broken",
    JSON.stringify({ type: "paragraph", content: [] }),
    JSON.stringify({ type: "doc", content: "not-an-array" }),
  ])("rejects malformed ProseMirror content", async (body) => {
    mocks.execute.mockResolvedValue([sourceRow({ body })]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).rejects.toThrow("malformed");
  });
});
