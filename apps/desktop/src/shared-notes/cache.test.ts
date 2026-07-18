import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    async (
      _statements: Array<{ sql: string; params?: unknown[] }>,
    ): Promise<number[]> => [],
  ),
  liveQueryExecute: vi.fn(),
  useDrizzleLiveQuery: vi.fn(
    (
      _query: { toSQL: () => { sql: string; params: unknown[] } },
      _options: { enabled?: boolean },
    ) => ({
      data: [] as unknown,
      error: null,
      isLoading: false,
    }),
  ),
}));

vi.mock("~/db", async () => {
  const { createDb } =
    await vi.importActual<typeof import("@hypr/db")>("@hypr/db");
  return {
    db: createDb({ executeProxy: vi.fn() }),
    executeTransaction: mocks.executeTransaction,
    liveQueryClient: { execute: mocks.liveQueryExecute },
    useDrizzleLiveQuery: mocks.useDrizzleLiveQuery,
  };
});

import {
  captureDurableSharedNoteCacheMutationVersion,
  loadManagedSharedNoteForSession,
  mapSharedNoteLiveRows,
  parseDurableSharedNoteSnapshots,
  removeDurableSharedNoteCache,
  replaceDurableSharedNoteCache,
  upsertDurableSharedNoteCache,
  useDurableSharedNote,
  useDurableSharedNotes,
} from "./cache";

const attachment = {
  id: "33333333-3333-4333-8333-333333333333",
  filename: "diagram.png",
  contentType: "image/png",
  sizeBytes: 42,
  sha256: "a".repeat(64),
};

const serverRow = {
  share_id: "11111111-1111-4111-8111-111111111111",
  workspace_id: "22222222-2222-4222-8222-222222222222",
  session_id: "session-1",
  schema_version: 1,
  content_revision: 3,
  title: "Shared plan",
  body_json: {
    type: "doc",
    content: [{ type: "paragraph" }],
  },
  attachments_json: [attachment],
  capability: "commenter",
  manage_access: false,
  access_version: 4,
  web_editable: false,
  web_edit_base_content_revision: null,
  web_edit_base_title: null,
  web_edit_base_body_json: null,
  published_at: "2026-07-16T17:30:00.000Z",
};

describe("durable shared-note cache", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("validates and maps the complete server snapshot", () => {
    expect(parseDurableSharedNoteSnapshots([serverRow])).toEqual([
      {
        shareId: serverRow.share_id,
        workspaceId: serverRow.workspace_id,
        sessionId: "session-1",
        schemaVersion: 1,
        contentRevision: 3,
        title: "Shared plan",
        body: serverRow.body_json,
        attachments: [attachment],
        capability: "commenter",
        manageAccess: false,
        accessVersion: 4,
        webEditable: false,
        webEditBase: null,
        publishedAt: "2026-07-16T17:30:00.000Z",
      },
    ]);
  });

  it("rejects duplicate or malformed snapshots before writes", () => {
    const { attachments_json: _attachments, ...missingAttachments } = serverRow;
    expect(() =>
      parseDurableSharedNoteSnapshots([serverRow, serverRow]),
    ).toThrow("duplicate");
    expect(() => parseDurableSharedNoteSnapshots([missingAttachments])).toThrow(
      "attachments",
    );
    expect(() =>
      parseDurableSharedNoteSnapshots([
        { ...serverRow, body_json: { type: "paragraph" } },
      ]),
    ).toThrow("document");
    expect(() =>
      parseDurableSharedNoteSnapshots([{ ...serverRow, capability: "owner" }]),
    ).toThrow("capability");
  });

  it("atomically replaces only the signed-in viewer's durable rows", async () => {
    const snapshot = parseDurableSharedNoteSnapshots([serverRow])[0]!;
    await replaceDurableSharedNoteCache("viewer-1", [snapshot]);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(5);
    expect(statements[0]?.sql).toContain(
      "UPDATE shared_session_attachment_cache",
    );
    expect(statements[1]).toEqual({
      sql: "DELETE FROM shared_session_cache WHERE viewer_user_id = ?",
      params: ["viewer-1"],
    });
    expect(statements[2].sql).toContain("INSERT INTO shared_session_cache");
    expect(statements[2].params).toEqual([
      serverRow.share_id,
      "viewer-1",
      serverRow.workspace_id,
      "session-1",
      1,
      3,
      "Shared plan",
      JSON.stringify(serverRow.body_json),
      JSON.stringify([attachment]),
      "commenter",
      0,
      4,
      0,
      null,
      null,
      null,
      "2026-07-16T17:30:00.000Z",
    ]);
    expect(statements[3].sql).toContain(
      "INSERT INTO shared_session_attachment_cache",
    );
    expect(statements[3].params).toContain(attachment.id);
    expect(statements[4]).toEqual({
      sql: expect.stringContaining("DELETE FROM session_share_sync_state"),
      params: ["viewer-1", serverRow.share_id],
    });
  });

  it("deletes revoked rows after a successful empty response", async () => {
    await replaceDurableSharedNoteCache("viewer-1", []);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(3);
    expect(statements[0]?.sql).toContain("availability = 'delete_pending'");
    expect(statements[1]).toEqual({
      sql: "DELETE FROM shared_session_cache WHERE viewer_user_id = ?",
      params: ["viewer-1"],
    });
    expect(statements[2]).toEqual({
      sql: "DELETE FROM session_share_sync_state WHERE viewer_user_id = ?",
      params: ["viewer-1"],
    });
  });

  it("upserts one account snapshot without replacing other rows", async () => {
    const snapshot = parseDurableSharedNoteSnapshots([serverRow])[0]!;

    await upsertDurableSharedNoteCache("viewer-1", snapshot);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(3);
    expect(statements[1]?.sql).toContain(
      "ON CONFLICT(viewer_user_id, share_id) DO UPDATE",
    );
    expect(statements[1]?.sql).not.toContain(
      "DELETE FROM shared_session_cache",
    );
    expect(statements[1]?.params).toContain("viewer-1");
    expect(statements[1]?.params).toContain(serverRow.share_id);
    expect(statements[2]?.sql).toContain(
      "ON CONFLICT(viewer_user_id, share_id, attachment_id)",
    );
  });

  it("removes only one viewer-owned cache row", async () => {
    await removeDurableSharedNoteCache("viewer-1", serverRow.share_id);

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(3);
    expect(statements[0]).toEqual({
      sql: expect.stringContaining("availability = 'delete_pending'"),
      params: ["viewer-1", serverRow.share_id],
    });
    expect(statements[1]).toEqual({
      sql: expect.stringContaining("WHERE viewer_user_id = ? AND share_id = ?"),
      params: ["viewer-1", serverRow.share_id],
    });
    expect(statements[2]).toEqual({
      sql: expect.stringContaining("DELETE FROM session_share_sync_state"),
      params: ["viewer-1", serverRow.share_id],
    });
  });

  it("loads the durable owner mapping used by fail-safe deletion", async () => {
    mocks.liveQueryExecute.mockResolvedValueOnce([
      {
        share_id: serverRow.share_id,
        workspace_id: serverRow.workspace_id,
        session_id: "session-1",
      },
    ]);

    await expect(
      loadManagedSharedNoteForSession("viewer-1", "session-1"),
    ).resolves.toEqual({
      shareId: serverRow.share_id,
      workspaceId: serverRow.workspace_id,
      sessionId: "session-1",
    });
    expect(mocks.liveQueryExecute).toHaveBeenCalledWith(
      expect.stringContaining("manage_access = 1"),
      ["viewer-1", "session-1"],
    );
  });

  it("serializes replacements for the same viewer", async () => {
    let finishFirstWrite: (() => void) | undefined;
    mocks.executeTransaction
      .mockImplementationOnce(
        () =>
          new Promise<number[]>((resolve) => {
            finishFirstWrite = () => resolve([]);
          }),
      )
      .mockResolvedValueOnce([]);
    const firstSnapshot = parseDurableSharedNoteSnapshots([serverRow])[0]!;
    const secondSnapshot = {
      ...firstSnapshot,
      contentRevision: 4,
      title: "Updated plan",
    };

    const first = replaceDurableSharedNoteCache("viewer-1", [firstSnapshot]);
    await vi.waitFor(() =>
      expect(mocks.executeTransaction).toHaveBeenCalledTimes(1),
    );
    const second = replaceDurableSharedNoteCache("viewer-1", [secondSnapshot]);
    await Promise.resolve();
    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);

    finishFirstWrite?.();
    await first;
    await second;

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(2);
    expect(mocks.executeTransaction.mock.calls[1]![0][2]?.params).toContain(
      "Updated plan",
    );
  });

  it("skips a stale full replacement after a newer local cache mutation", async () => {
    const snapshot = parseDurableSharedNoteSnapshots([serverRow])[0]!;
    const capturedVersion =
      captureDurableSharedNoteCacheMutationVersion("viewer-1");
    await upsertDurableSharedNoteCache("viewer-1", {
      ...snapshot,
      contentRevision: 4,
      title: "Newer local publish",
    });
    mocks.executeTransaction.mockClear();

    await expect(
      replaceDurableSharedNoteCache("viewer-1", [snapshot], capturedVersion),
    ).resolves.toBe(false);
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("maps raw SQLite JSON and boolean values", () => {
    expect(
      mapSharedNoteLiveRows([
        {
          share_id: serverRow.share_id,
          viewer_user_id: "viewer-1",
          workspace_id: serverRow.workspace_id,
          session_id: "session-1",
          schema_version: 1,
          content_revision: 3,
          title: "Shared plan",
          body_json: JSON.stringify(serverRow.body_json),
          attachments_json: JSON.stringify([attachment]),
          capability: "editor",
          manage_access: 1,
          access_version: 4,
          web_editable: 0,
          web_edit_base_content_revision: null,
          web_edit_base_title: null,
          web_edit_base_body_json: null,
          published_at: "2026-07-16T17:30:00.000Z",
          cached_at: "2026-07-16T17:31:00.000Z",
        },
      ]),
    ).toEqual([
      expect.objectContaining({
        body: serverRow.body_json,
        capability: "editor",
        manageAccess: true,
      }),
    ]);
  });

  it("admits manager-only pending web edit bases and rejects incomplete tuples", () => {
    const pending = {
      ...serverRow,
      attachments_json: [],
      manage_access: true,
      capability: "editor",
      content_revision: 4,
      web_editable: true,
      web_edit_base_content_revision: 3,
      web_edit_base_title: "Shared plan",
      web_edit_base_body_json: serverRow.body_json,
    };
    expect(parseDurableSharedNoteSnapshots([pending])[0]).toMatchObject({
      webEditable: true,
      webEditBase: {
        contentRevision: 3,
        title: "Shared plan",
        body: serverRow.body_json,
      },
    });
    expect(() =>
      parseDurableSharedNoteSnapshots([
        { ...pending, web_edit_base_title: null },
      ]),
    ).toThrow("web edit base");
    expect(() =>
      parseDurableSharedNoteSnapshots([{ ...pending, manage_access: false }]),
    ).toThrow("web edit base");
  });

  it("preserves attachment manifests on web-editable snapshots", () => {
    expect(
      parseDurableSharedNoteSnapshots([
        { ...serverRow, web_editable: true },
      ])[0],
    ).toMatchObject({
      webEditable: true,
      attachments: [attachment],
    });
  });

  it("scopes list and detail live queries to the signed-in viewer", () => {
    renderHook(() => useDurableSharedNotes("viewer-a"));
    const [listQuery, listOptions] = mocks.useDrizzleLiveQuery.mock.calls[0]!;
    expect(listQuery.toSQL().sql).toContain("viewer_user_id");
    expect(listQuery.toSQL().params).toContain("viewer-a");
    expect(listOptions.enabled).toBe(true);

    mocks.useDrizzleLiveQuery.mockClear();
    renderHook(() => useDurableSharedNote("viewer-b", serverRow.share_id));
    const [detailQuery, detailOptions] =
      mocks.useDrizzleLiveQuery.mock.calls[0]!;
    expect(detailQuery.toSQL().sql).toContain("viewer_user_id");
    expect(detailQuery.toSQL().sql).toContain("share_id");
    expect(detailQuery.toSQL().params).toEqual(
      expect.arrayContaining(["viewer-b", serverRow.share_id]),
    );
    expect(detailOptions.enabled).toBe(true);
  });

  it("disables live reads and returns no rows while signed out", () => {
    mocks.useDrizzleLiveQuery.mockReturnValueOnce({
      data: [parseDurableSharedNoteSnapshots([serverRow])[0]],
      error: null,
      isLoading: false,
    });

    const { result } = renderHook(() => useDurableSharedNotes(null));
    const [, options] = mocks.useDrizzleLiveQuery.mock.calls[0]!;
    expect(options.enabled).toBe(false);
    expect(result.current).toEqual([]);
  });
});
