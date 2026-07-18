import { beforeEach, describe, expect, it, vi } from "vitest";

import type { JSONContent } from "@hypr/editor/note";

const mocks = vi.hoisted(() => ({
  enqueueDatabaseWrite: vi.fn((_key: string, write: () => Promise<unknown>) =>
    write(),
  ),
  executeTransaction: vi.fn<(statements: Array<any>) => Promise<number[]>>(
    async () => [],
  ),
  liveQueryExecute:
    vi.fn<(sql: string, params?: unknown[]) => Promise<any[]>>(),
  loadAttachments: vi.fn<() => Promise<any[]>>().mockResolvedValue([]),
  loadSource: vi.fn<() => Promise<any>>(),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.liveQueryExecute },
}));
vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
  flushDatabaseWrites: vi.fn(async () => {}),
}));
vi.mock("./source", () => ({
  loadSessionShareSource: mocks.loadSource,
}));
vi.mock("./attachments", async () => {
  const actual =
    await vi.importActual<typeof import("./attachments")>("./attachments");
  return { ...actual, loadSessionShareAttachments: mocks.loadAttachments };
});

import type { SessionShareAttachment } from "./attachments";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  reconcileManagedSessionShareSnapshot,
} from "./reconciliation";

import type { SharedNoteSnapshot } from "~/shared-notes/cache";

const VIEWER_ID = "11111111-1111-4111-8111-111111111111";
const WORKSPACE_ID = "22222222-2222-4222-8222-222222222222";
const SHARE_ID = "33333333-3333-4333-8333-333333333333";
const SESSION_ID = "session-1";
const SHARED_ATTACHMENT_ID = "44444444-4444-4444-8444-444444444444";
const LOCAL_ATTACHMENT_ID = "local-attachment";
const LOCAL_ATTACHMENT_SOURCE_ID = "diagram.png";
const baseBody: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "Base" }] }],
};
const remoteBody: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "Remote" }] }],
};
const localBody: JSONContent = {
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text: "Local" }] }],
};
const sharedAttachment = {
  id: SHARED_ATTACHMENT_ID,
  filename: "diagram.png",
  contentType: "image/png",
  sizeBytes: 42,
  sha256: "a".repeat(64),
};
const localAttachment: SessionShareAttachment = {
  id: LOCAL_ATTACHMENT_ID,
  filename: sharedAttachment.filename,
  contentType: sharedAttachment.contentType,
  sizeBytes: sharedAttachment.sizeBytes,
  sha256: sharedAttachment.sha256,
  sourceType: "note_upload",
  sourceId: LOCAL_ATTACHMENT_SOURCE_ID,
  cloudSyncEnabled: true,
  cloudObjectKey:
    "11111111-1111-4111-8111-111111111111/22222222-2222-4222-8222-222222222222.anb1",
  localAvailability: "present",
  transferDirection: null,
  transferPhase: "completed",
  transferError: "",
};
const localImageAttrs = {
  attachmentId: LOCAL_ATTACHMENT_SOURCE_ID,
  src: "asset://local/diagram.png",
  alt: "Architecture diagram",
  title: "System overview",
  editorWidth: 42,
};
const attachedBaseBody: JSONContent = {
  type: "doc",
  content: [
    { type: "image", attrs: { sharedAttachmentId: SHARED_ATTACHMENT_ID } },
    { type: "paragraph", content: [{ type: "text", text: "Base" }] },
  ],
};
const attachedRemoteBody: JSONContent = {
  type: "doc",
  content: [
    { type: "image", attrs: { sharedAttachmentId: SHARED_ATTACHMENT_ID } },
    { type: "paragraph", content: [{ type: "text", text: "Remote" }] },
  ],
};
const localAttachedBaseBody: JSONContent = {
  type: "doc",
  content: [
    { type: "image", attrs: localImageAttrs },
    { type: "paragraph", content: [{ type: "text", text: "Base" }] },
  ],
};
const localAttachedRemoteBody: JSONContent = {
  type: "doc",
  content: [
    { type: "image", attrs: localImageAttrs },
    { type: "paragraph", content: [{ type: "text", text: "Remote" }] },
  ],
};

function snapshot(
  overrides: Partial<SharedNoteSnapshot> = {},
): SharedNoteSnapshot {
  return {
    shareId: SHARE_ID,
    workspaceId: WORKSPACE_ID,
    sessionId: SESSION_ID,
    schemaVersion: 1,
    contentRevision: 1,
    title: "Base title",
    body: baseBody,
    attachments: [],
    capability: "editor",
    manageAccess: true,
    accessVersion: 1,
    webEditable: true,
    webEditBase: null,
    publishedAt: "2026-07-17T00:00:00.000Z",
    ...overrides,
  };
}

function source(title = "Base title", body: JSONContent = baseBody) {
  return {
    sessionId: SESSION_ID,
    documentId: SESSION_ID,
    workspaceId: WORKSPACE_ID,
    title,
    body,
    rawBody: JSON.stringify(body),
    bodyFormat: "prosemirror_json",
  };
}

function stateRow(input: {
  revision: number;
  hash: string;
  status?: "clean" | "conflict";
}) {
  return {
    viewer_user_id: VIEWER_ID,
    share_id: SHARE_ID,
    session_id: SESSION_ID,
    acknowledged_content_revision: input.revision,
    baseline_source_hash: input.hash,
    status: input.status ?? "clean",
  };
}

describe("session share reconciliation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadSource.mockResolvedValue(source());
    mocks.loadAttachments.mockResolvedValue([]);
    mocks.liveQueryExecute.mockResolvedValue([]);
    mocks.executeTransaction.mockResolvedValue([]);
    mocks.enqueueDatabaseWrite.mockImplementation((_key, write) => write());
  });

  it("hashes sorted-key projections and derives a stable v4 mutation ID", async () => {
    const left = await hashSessionShareProjection({
      title: " Title ",
      body: { type: "doc", attrs: { z: 1, a: 2 }, content: [] },
    });
    const right = await hashSessionShareProjection({
      title: "Title",
      body: { content: [], attrs: { a: 2, z: 1 }, type: "doc" },
    });
    expect(left).toBe(right);

    const first = await createSessionShareMutationId({
      shareId: SHARE_ID,
      baseRevision: 2,
      sourceHash: left,
    });
    const retry = await createSessionShareMutationId({
      shareId: SHARE_ID,
      baseRevision: 2,
      sourceHash: left,
    });
    const nextRevision = await createSessionShareMutationId({
      shareId: SHARE_ID,
      baseRevision: 3,
      sourceHash: left,
    });
    const attachmentChange = await createSessionShareMutationId({
      shareId: SHARE_ID,
      baseRevision: 2,
      sourceHash: left,
      attachmentIds: ["44444444-4444-4444-8444-444444444444"],
    });
    expect(first).toBe(retry);
    expect(first).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(nextRevision).not.toBe(first);
    expect(attachmentChange).not.toBe(first);
  });

  it("keeps an unchanged acknowledged snapshot idle", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot(),
      }),
    ).resolves.toBe("idle");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("defers reconciliation while a focused session editor is active", async () => {
    const acknowledge = vi.fn(async () => {});
    const isSessionEditorActive = vi.fn(async () => true);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: baseBody,
          },
        }),
        acknowledge,
        isSessionEditorActive,
      }),
    ).resolves.toBe("deferred");

    expect(isSessionEditorActive).toHaveBeenCalledWith(SESSION_ID);
    expect(mocks.loadSource).not.toHaveBeenCalled();
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("requests a legacy assessment publish without recording clean state", async () => {
    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({ webEditable: false }),
      }),
    ).resolves.toBe("assessment_required");

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("defers a pending editor write before its 500ms persistence debounce", async () => {
    vi.useFakeTimers();
    const persistPendingChange = vi.fn();
    const acknowledge = vi.fn(async () => {});
    const isSessionEditorActive = vi.fn(async () => true);
    setTimeout(persistPendingChange, 500);

    await vi.advanceTimersByTimeAsync(499);
    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: baseBody,
          },
        }),
        acknowledge,
        isSessionEditorActive,
      }),
    ).resolves.toBe("deferred");

    expect(persistPendingChange).not.toHaveBeenCalled();
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
    expect(acknowledge).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("holds the activation interlock across the native import transaction", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);
    const acknowledge = vi.fn(async () => {});
    const isSessionEditorActive = vi.fn(async () => false);
    let importLocked = false;
    const releaseImportLock = vi.fn(() => {
      importLocked = false;
    });
    const acquireSessionImportLock = vi.fn(() => {
      importLocked = true;
      return releaseImportLock;
    });
    mocks.executeTransaction.mockImplementationOnce(async () => {
      expect(importLocked).toBe(true);
      return [];
    });

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: baseBody,
          },
        }),
        acknowledge,
        isSessionEditorActive,
        acquireSessionImportLock,
      }),
    ).resolves.toBe("imported");

    expect(acquireSessionImportLock).toHaveBeenCalledWith(SESSION_ID);
    expect(releaseImportLock).toHaveBeenCalledOnce();
    expect(importLocked).toBe(false);
    expect(acknowledge).toHaveBeenCalledWith(SHARE_ID, 2);
  });

  it("cancels an import while its database write is still queued", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);
    let runQueuedWrite: (() => void) | undefined;
    mocks.enqueueDatabaseWrite.mockImplementationOnce(
      (_key, write) =>
        new Promise<unknown>((resolve, reject) => {
          runQueuedWrite = () => {
            void write().then(resolve, reject);
          };
        }),
    );
    const controller = new AbortController();
    const acknowledge = vi.fn(async () => {});
    const reconciliation = reconcileManagedSessionShareSnapshot({
      viewerUserId: VIEWER_ID,
      snapshot: snapshot({
        contentRevision: 2,
        title: "Remote title",
        body: remoteBody,
      }),
      signal: controller.signal,
      acknowledge,
    });

    await vi.waitFor(() => {
      expect(runQueuedWrite).toBeTypeOf("function");
    });
    const rejected = expect(reconciliation).rejects.toMatchObject({
      name: "AbortError",
    });
    controller.abort();
    runQueuedWrite?.();

    await rejected;
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("leaves a local-only change pending for the CAS publisher", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.loadSource.mockResolvedValue(source("Local title", localBody));
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot(),
      }),
    ).resolves.toBe("local_pending");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("recovers when a newer cloud snapshot already matches local content", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    const remoteHash = await hashSessionShareProjection({
      title: "Remote title",
      body: remoteBody,
    });
    mocks.loadSource.mockResolvedValue(source("Remote title", remoteBody));
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
        }),
      }),
    ).resolves.toBe("idle");

    const [statement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(statement.sql).toContain("session_share_sync_state");
    expect(statement.sql).not.toContain("UPDATE sessions");
    expect(statement.params).toEqual(
      expect.arrayContaining([2, remoteHash, "clean"]),
    );
  });

  it("imports a remote-only change with exact local CAS predicates and then acknowledges", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);
    const acknowledge = vi.fn(async () => {});

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: baseBody,
          },
        }),
        acknowledge,
      }),
    ).resolves.toBe("imported");

    const statements = mocks.executeTransaction.mock.calls[0]![0];
    expect(statements).toHaveLength(3);
    expect(statements[0]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[0].sql).toContain("AND title = ?");
    expect(statements[1]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[1].sql).toContain("AND body = ?");
    expect(statements[1].sql).toContain("AND body_format = ?");
    expect(statements[2].params).toContain("clean");
    expect(acknowledge).toHaveBeenCalledWith(SHARE_ID, 2);
    expect(mocks.executeTransaction.mock.invocationCallOrder[0]).toBeLessThan(
      acknowledge.mock.invocationCallOrder[0]!,
    );
  });

  it("imports a remote-only edit while preserving its attachment manifest", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: attachedBaseBody,
    });
    mocks.loadSource.mockResolvedValue(
      source("Base title", localAttachedBaseBody),
    );
    mocks.loadAttachments.mockResolvedValue([localAttachment]);
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: attachedRemoteBody,
          attachments: [sharedAttachment],
        }),
      }),
    ).resolves.toBe("imported");

    const nextBody = mocks.executeTransaction.mock.calls[0]![0][1].params[0];
    expect(JSON.parse(nextBody)).toEqual(localAttachedRemoteBody);
    expect(nextBody).not.toContain(SHARED_ATTACHMENT_ID);
  });

  it("makes a stale import state write fail closed behind a newer revision", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await reconcileManagedSessionShareSnapshot({
      viewerUserId: VIEWER_ID,
      snapshot: snapshot({
        contentRevision: 2,
        title: "Remote title",
        body: remoteBody,
      }),
    });

    const stateStatement = mocks.executeTransaction.mock.calls[0]![0][2];
    expect(stateStatement).toMatchObject({ expectedRowsAffected: 1 });
    expect(stateStatement.sql).toContain(
      "WHERE session_share_sync_state.acknowledged_content_revision",
    );
    expect(stateStatement.sql).toContain(
      "<= excluded.acknowledged_content_revision",
    );
  });

  it("marks both-changed content conflicting without overwriting local content", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.loadSource.mockResolvedValue(source("Local title", localBody));
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
        }),
      }),
    ).resolves.toBe("conflict");

    const [statement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(statement.sql).toContain("session_share_sync_state");
    expect(statement.params).toContain("conflict");
    expect(statement.sql).not.toContain("UPDATE sessions");
  });

  it("bootstraps a pending web edit only when local content matches its server base", async () => {
    const acknowledge = vi.fn(async () => {});
    const pending = snapshot({
      contentRevision: 2,
      title: "Remote title",
      body: remoteBody,
      webEditBase: {
        contentRevision: 1,
        title: "Base title",
        body: baseBody,
      },
    });

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: pending,
        acknowledge,
      }),
    ).resolves.toBe("imported");
    expect(acknowledge).toHaveBeenCalledWith(SHARE_ID, 2);

    vi.clearAllMocks();
    mocks.loadSource.mockResolvedValue(source("Local title", localBody));
    mocks.loadAttachments.mockResolvedValue([]);
    mocks.liveQueryExecute.mockResolvedValue([]);
    mocks.executeTransaction.mockResolvedValue([]);
    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: pending,
      }),
    ).resolves.toBe("conflict");
    expect(mocks.executeTransaction.mock.calls[0]![0][0].sql).not.toContain(
      "UPDATE sessions",
    );
  });

  it("bootstraps a pending web edit with a preserved attachment manifest", async () => {
    mocks.loadSource.mockResolvedValue(
      source("Base title", localAttachedBaseBody),
    );
    mocks.loadAttachments.mockResolvedValue([localAttachment]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: attachedRemoteBody,
          attachments: [sharedAttachment],
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: attachedBaseBody,
          },
        }),
      }),
    ).resolves.toBe("imported");

    const nextBody = mocks.executeTransaction.mock.calls[0]![0][1].params[0];
    expect(JSON.parse(nextBody)).toEqual(localAttachedRemoteBody);
  });

  it("records a bootstrap conflict when an attachment disappears before import", async () => {
    const baseHash = await hashSessionShareProjection({
      title: "Base title",
      body: attachedBaseBody,
    });
    mocks.loadSource.mockResolvedValue(
      source("Base title", localAttachedBaseBody),
    );
    mocks.loadAttachments
      .mockResolvedValueOnce([localAttachment])
      .mockResolvedValueOnce([]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: attachedRemoteBody,
          attachments: [sharedAttachment],
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: attachedBaseBody,
          },
        }),
      }),
    ).resolves.toBe("conflict");

    const [stateStatement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(stateStatement.sql).toContain("session_share_sync_state");
    expect(stateStatement.params).toEqual(
      expect.arrayContaining([1, baseHash, "conflict"]),
    );
  });

  it("acknowledges a pending snapshot already reflected locally without importing again", async () => {
    mocks.loadSource.mockResolvedValue(source("Remote title", remoteBody));
    const acknowledge = vi.fn(async () => {});

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
          webEditBase: {
            contentRevision: 1,
            title: "Base title",
            body: baseBody,
          },
        }),
        acknowledge,
      }),
    ).resolves.toBe("idle");
    expect(mocks.executeTransaction.mock.calls[0]![0][0].sql).toContain(
      "session_share_sync_state",
    );
    expect(mocks.executeTransaction.mock.calls[0]![0][0].sql).not.toContain(
      "UPDATE sessions",
    );
    expect(acknowledge).toHaveBeenCalledWith(SHARE_ID, 2);
  });

  it("fails closed on a second device after another device removed the pending base", async () => {
    const currentSnapshot = snapshot({
      contentRevision: 2,
      title: "Remote title",
      body: remoteBody,
      webEditBase: null,
    });
    const currentHash = await hashSessionShareProjection({
      title: currentSnapshot.title,
      body: currentSnapshot.body,
    });
    const acknowledge = vi.fn(async () => {});

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: currentSnapshot,
        acknowledge,
      }),
    ).resolves.toBe("conflict");

    const [conflictStatement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(conflictStatement.sql).toContain("session_share_sync_state");
    expect(conflictStatement.sql).not.toContain("UPDATE sessions");
    expect(conflictStatement.params).toEqual(
      expect.arrayContaining([2, currentHash, "conflict"]),
    );
    expect(acknowledge).not.toHaveBeenCalled();

    mocks.executeTransaction.mockClear();
    mocks.loadSource.mockResolvedValue(source("Remote title", remoteBody));
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 2, hash: currentHash, status: "conflict" }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: currentSnapshot,
        acknowledge,
      }),
    ).resolves.toBe("idle");

    const [recoveredStatement] = mocks.executeTransaction.mock.calls[0]![0];
    expect(recoveredStatement.params).toEqual(
      expect.arrayContaining([2, currentHash, "clean"]),
    );
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("rolls back an import race and never acknowledges it", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);
    mocks.executeTransaction.mockRejectedValueOnce(
      new Error("unexpected rows affected"),
    );
    const acknowledge = vi.fn(async () => {});

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
        }),
        acknowledge,
      }),
    ).rejects.toThrow("unexpected rows affected");
    expect(acknowledge).not.toHaveBeenCalled();
  });

  it("CAS-updates the exact legacy fallback document instead of forcing a permanent conflict", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.loadSource.mockResolvedValue({
      ...source(),
      documentId: "legacy-document-1",
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
        }),
      }),
    ).resolves.toBe("imported");
    expect(mocks.executeTransaction.mock.calls[0]![0][1].params).toEqual(
      expect.arrayContaining(["legacy-document-1", SESSION_ID]),
    );
  });

  it("recovers a conflict when the local projection is restored to the baseline", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: baseBody,
    });
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline, status: "conflict" }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: remoteBody,
        }),
      }),
    ).resolves.toBe("imported");
    expect(mocks.executeTransaction.mock.calls[0]![0][0].sql).toContain(
      "UPDATE sessions",
    );
  });

  it("recovers an attachment-bearing edit after local content returns to baseline", async () => {
    const baseline = await hashSessionShareProjection({
      title: "Base title",
      body: attachedBaseBody,
    });
    mocks.loadSource.mockResolvedValue(
      source("Base title", localAttachedBaseBody),
    );
    mocks.loadAttachments.mockResolvedValue([localAttachment]);
    mocks.liveQueryExecute.mockResolvedValue([
      stateRow({ revision: 1, hash: baseline, status: "conflict" }),
    ]);

    await expect(
      reconcileManagedSessionShareSnapshot({
        viewerUserId: VIEWER_ID,
        snapshot: snapshot({
          contentRevision: 2,
          title: "Remote title",
          body: attachedRemoteBody,
          attachments: [sharedAttachment],
        }),
      }),
    ).resolves.toBe("imported");

    const nextBody = mocks.executeTransaction.mock.calls[0]![0][1].params[0];
    expect(JSON.parse(nextBody)).toEqual(localAttachedRemoteBody);
  });
});
