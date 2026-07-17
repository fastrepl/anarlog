import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  flushDatabaseWrites: vi.fn().mockResolvedValue(undefined),
  workspaceRows: [] as Array<{ id: string; name: string }>,
  liveQueryOptions: null as null | {
    sql: string;
    params: unknown[];
    enabled: boolean;
  },
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWrites: mocks.flushDatabaseWrites,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: ({ sql, params, enabled, mapRows }: any) => {
    mocks.liveQueryOptions = { sql, params, enabled };
    return { data: mapRows(mocks.workspaceRows) };
  },
}));

import { loadSessionShareSource, useAvailableShareWorkspaces } from "./source";

import { DEFAULT_USER_ID } from "~/shared/utils";

const ACCOUNT_ID = "account-1";

function sourceRow(
  overrides: Partial<{
    id: string;
    workspace_id: string;
    title: string;
    body: string;
    body_format: string;
    personal_workspace_available: number | boolean;
    assigned_workspace_kind: string | null;
    assigned_workspace_deleted_at: string | null;
    assigned_workspace_role: string | null;
    binding_json: string | null;
  }> = {},
) {
  return {
    id: "session-1",
    workspace_id: ACCOUNT_ID,
    title: "Planning notes",
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
    mocks.workspaceRows = [];
    mocks.liveQueryOptions = null;
  });

  it("loads the current personal-workspace note", async () => {
    mocks.execute.mockResolvedValue([sourceRow()]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toEqual({
      sessionId: "session-1",
      workspaceId: ACCOUNT_ID,
      title: "Planning notes",
      body: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Current content" }],
          },
        ],
      },
    });

    const [sql, params] = mocks.execute.mock.calls[0]!;
    expect(sql).toContain("fallback.updated_at DESC");
    expect(params).toEqual([
      ACCOUNT_ID,
      ACCOUNT_ID,
      ACCOUNT_ID,
      ACCOUNT_ID,
      "session-1",
    ]);
    expect(mocks.flushDatabaseWrites).toHaveBeenCalledOnce();
    expect(mocks.flushDatabaseWrites.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.execute.mock.invocationCallOrder[0]!,
    );
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

  it("returns the canonical empty document for an empty local note", async () => {
    mocks.execute.mockResolvedValue([sourceRow({ body: "" })]);

    await expect(
      loadSessionShareSource("session-1", ACCOUNT_ID),
    ).resolves.toMatchObject({ body: { type: "doc", content: [] } });
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

describe("useAvailableShareWorkspaces", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.workspaceRows = [];
    mocks.liveQueryOptions = null;
  });

  it("returns every active shared-workspace membership", () => {
    mocks.workspaceRows = [
      { id: "workspace-a", name: "Acme" },
      { id: "workspace-b", name: "Beta" },
    ];

    expect(
      renderHook(() => useAvailableShareWorkspaces(ACCOUNT_ID)).result.current,
    ).toEqual(mocks.workspaceRows);
    expect(mocks.liveQueryOptions).toMatchObject({
      params: [ACCOUNT_ID],
      enabled: true,
    });
    expect(mocks.liveQueryOptions?.sql).toContain("workspace.kind = 'shared'");
    expect(mocks.liveQueryOptions?.sql).toContain(
      "membership.deleted_at IS NULL",
    );
    expect(mocks.liveQueryOptions?.sql).not.toContain("membership.role IN");
  });

  it("disables the query without a signed-in account", () => {
    expect(
      renderHook(() => useAvailableShareWorkspaces(null)).result.current,
    ).toEqual([]);
    expect(mocks.liveQueryOptions).toMatchObject({ enabled: false });
  });
});
