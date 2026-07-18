import type { JSONContent } from "@hypr/editor/note";

import {
  addSharedAttachmentIds,
  loadSessionShareAttachments,
  matchSharedAttachmentsToLocal,
  restoreLocalAttachmentIds,
} from "./attachments";
import { loadSessionShareSource } from "./source";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite, flushDatabaseWrites } from "~/db/write-queue";
import {
  enqueueDurableSharedNoteCacheMutation,
  type SharedNoteSnapshot,
} from "~/shared-notes/cache";

const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export type SessionShareSyncState = {
  viewerUserId: string;
  shareId: string;
  sessionId: string;
  acknowledgedContentRevision: number;
  baselineSourceHash: string;
  status: "clean" | "conflict";
};

type SessionShareSyncStateSqlRow = {
  viewer_user_id: string;
  share_id: string;
  session_id: string;
  acknowledged_content_revision: number;
  baseline_source_hash: string;
  status: string;
};

export type ManagedShareProjection = {
  source: Awaited<ReturnType<typeof loadSessionShareSource>>;
  body: JSONContent;
  hash: string;
};

export type ReconciliationOutcome =
  | "ignored"
  | "deferred"
  | "idle"
  | "local_pending"
  | "assessment_required"
  | "imported"
  | "conflict";

export async function hashSessionShareProjection(input: {
  title: string;
  body: JSONContent;
}): Promise<string> {
  const bytes = new TextEncoder().encode(
    canonicalJson({ title: input.title.trim(), body: input.body }),
  );
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

export async function createSessionShareMutationId(input: {
  shareId: string;
  baseRevision: number;
  sourceHash: string;
  attachmentIds?: string[];
}): Promise<string> {
  if (
    !Number.isSafeInteger(input.baseRevision) ||
    input.baseRevision < 0 ||
    !SHA256_PATTERN.test(input.sourceHash)
  ) {
    throw new Error("Invalid shared-note mutation source");
  }
  const digest = new Uint8Array(
    await crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(
        `anarlog-session-share-mutation-v1\0${input.shareId}\0${input.baseRevision}\0${input.sourceHash}\0${canonicalJson(input.attachmentIds ?? [])}`,
      ),
    ),
  );
  const bytes = digest.slice(0, 16);
  bytes[6] = (bytes[6]! & 0x0f) | 0x40;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export async function loadManagedShareProjection(
  viewerUserId: string,
  snapshot: Pick<
    SharedNoteSnapshot,
    "workspaceId" | "sessionId" | "attachments"
  >,
): Promise<ManagedShareProjection> {
  const source = await loadSessionShareSource(snapshot.sessionId, viewerUserId);
  if (
    source.sessionId !== snapshot.sessionId ||
    source.workspaceId !== snapshot.workspaceId
  ) {
    throw new Error("Shared note source changed");
  }
  const localAttachments = await loadSessionShareAttachments(
    snapshot.sessionId,
  );
  const localToShared = matchSharedAttachmentsToLocal(
    localAttachments,
    snapshot.attachments,
  );
  const body = addSharedAttachmentIds(
    source.body,
    localAttachments,
    localToShared,
  );
  return {
    source,
    body,
    hash: await hashSessionShareProjection({ title: source.title, body }),
  };
}

export async function loadSessionShareSyncState(
  viewerUserId: string,
  shareId: string,
): Promise<SessionShareSyncState | null> {
  await flushDatabaseWrites();
  const rows = await liveQueryClient.execute<SessionShareSyncStateSqlRow>(
    `
      SELECT
        viewer_user_id,
        share_id,
        session_id,
        acknowledged_content_revision,
        baseline_source_hash,
        status
      FROM session_share_sync_state
      WHERE viewer_user_id = ? AND share_id = ?
      LIMIT 1
    `,
    [viewerUserId, shareId],
  );
  const row = rows[0];
  if (!row) return null;
  if (
    !Number.isSafeInteger(row.acknowledged_content_revision) ||
    row.acknowledged_content_revision < 1 ||
    !SHA256_PATTERN.test(row.baseline_source_hash) ||
    (row.status !== "clean" && row.status !== "conflict")
  ) {
    throw new Error("Invalid shared-note sync state");
  }
  return {
    viewerUserId: row.viewer_user_id,
    shareId: row.share_id,
    sessionId: row.session_id,
    acknowledgedContentRevision: row.acknowledged_content_revision,
    baselineSourceHash: row.baseline_source_hash,
    status: row.status,
  };
}

export async function recordPublishedSessionShareState(input: {
  viewerUserId: string;
  shareId: string;
  sessionId: string;
  contentRevision: number;
  sourceHash: string;
}): Promise<void> {
  const state: SessionShareSyncState = {
    viewerUserId: input.viewerUserId,
    shareId: input.shareId,
    sessionId: input.sessionId,
    acknowledgedContentRevision: input.contentRevision,
    baselineSourceHash: input.sourceHash,
    status: "clean",
  };
  await enqueueDurableSharedNoteCacheMutation(input.viewerUserId, () =>
    executeTransaction([syncStateStatement(state)]),
  );
}

export async function reconcileManagedSessionShareSnapshot(input: {
  viewerUserId: string;
  snapshot: SharedNoteSnapshot;
  signal?: AbortSignal;
  acknowledge?: (shareId: string, contentRevision: number) => Promise<void>;
  isSessionEditorActive?: (sessionId: string) => boolean | Promise<boolean>;
  acquireSessionImportLock?: (sessionId: string) => (() => void) | null;
}): Promise<ReconciliationOutcome> {
  const { snapshot, viewerUserId } = input;
  input.signal?.throwIfAborted();
  if (!snapshot.manageAccess) return "ignored";
  const shouldDefer = await shouldDeferForActiveEditor(input);
  input.signal?.throwIfAborted();
  if (shouldDefer) return "deferred";

  const projection = await loadManagedShareProjection(viewerUserId, snapshot);
  input.signal?.throwIfAborted();
  const state = await loadSessionShareSyncState(viewerUserId, snapshot.shareId);
  input.signal?.throwIfAborted();
  if (state && state.sessionId !== snapshot.sessionId) {
    throw new Error("Shared note sync state changed sessions");
  }
  if (state && snapshot.contentRevision < state.acknowledgedContentRevision) {
    throw new Error("Shared note snapshot revision regressed");
  }

  const snapshotHash = await hashSessionShareProjection({
    title: snapshot.title,
    body: snapshot.body,
  });
  input.signal?.throwIfAborted();

  if (!state) {
    if (snapshot.webEditBase) {
      if (projection.hash === snapshotHash) {
        await writeSyncState(
          {
            viewerUserId,
            shareId: snapshot.shareId,
            sessionId: snapshot.sessionId,
            acknowledgedContentRevision: snapshot.contentRevision,
            baselineSourceHash: snapshotHash,
            status: "clean",
          },
          input.signal,
        );
        await bestEffortAcknowledge(input);
        return "idle";
      }
      const baseHash = await hashSessionShareProjection({
        title: snapshot.webEditBase.title,
        body: snapshot.webEditBase.body,
      });
      input.signal?.throwIfAborted();
      if (projection.hash !== baseHash || !snapshot.webEditable) {
        await writeSyncState(
          {
            viewerUserId,
            shareId: snapshot.shareId,
            sessionId: snapshot.sessionId,
            acknowledgedContentRevision: snapshot.webEditBase.contentRevision,
            baselineSourceHash: baseHash,
            status: "conflict",
          },
          input.signal,
        );
        return "conflict";
      }
      return importSnapshot({
        ...input,
        projection,
        snapshotHash,
        priorState: null,
      });
    }

    if (projection.hash !== snapshotHash) {
      await writeSyncState(
        {
          viewerUserId,
          shareId: snapshot.shareId,
          sessionId: snapshot.sessionId,
          acknowledgedContentRevision: snapshot.contentRevision,
          baselineSourceHash: snapshotHash,
          status: "conflict",
        },
        input.signal,
      );
      return "conflict";
    }

    if (!snapshot.webEditable) return "assessment_required";

    await writeSyncState(
      {
        viewerUserId,
        shareId: snapshot.shareId,
        sessionId: snapshot.sessionId,
        acknowledgedContentRevision: snapshot.contentRevision,
        baselineSourceHash: snapshotHash,
        status: "clean",
      },
      input.signal,
    );
    return "idle";
  }

  if (state.status === "conflict") {
    if (projection.hash === snapshotHash) {
      await writeSyncState(
        {
          ...state,
          acknowledgedContentRevision: snapshot.contentRevision,
          baselineSourceHash: snapshotHash,
          status: "clean",
        },
        input.signal,
      );
      await bestEffortAcknowledge(input);
      return "idle";
    }
    if (
      projection.hash === state.baselineSourceHash &&
      snapshot.contentRevision > state.acknowledgedContentRevision &&
      snapshot.webEditable
    ) {
      return importSnapshot({
        ...input,
        projection,
        snapshotHash,
        priorState: state,
      });
    }
    return "conflict";
  }
  if (snapshot.contentRevision === state.acknowledgedContentRevision) {
    if (snapshot.webEditable && snapshotHash !== state.baselineSourceHash) {
      throw new Error("Shared note snapshot changed without a revision");
    }
    if (snapshot.webEditBase) {
      await bestEffortAcknowledge(input);
    }
    return projection.hash === state.baselineSourceHash
      ? "idle"
      : "local_pending";
  }

  if (projection.hash === snapshotHash) {
    await writeSyncState(
      {
        ...state,
        acknowledgedContentRevision: snapshot.contentRevision,
        baselineSourceHash: snapshotHash,
        status: "clean",
      },
      input.signal,
    );
    await bestEffortAcknowledge(input);
    return "idle";
  }

  if (projection.hash !== state.baselineSourceHash || !snapshot.webEditable) {
    await writeSyncState({ ...state, status: "conflict" }, input.signal);
    return "conflict";
  }

  return importSnapshot({
    ...input,
    projection,
    snapshotHash,
    priorState: state,
  });
}

async function importSnapshot(input: {
  viewerUserId: string;
  snapshot: SharedNoteSnapshot;
  signal?: AbortSignal;
  projection: ManagedShareProjection;
  snapshotHash: string;
  priorState: SessionShareSyncState | null;
  acknowledge?: (shareId: string, contentRevision: number) => Promise<void>;
  isSessionEditorActive?: (sessionId: string) => boolean | Promise<boolean>;
  acquireSessionImportLock?: (sessionId: string) => (() => void) | null;
}): Promise<ReconciliationOutcome> {
  const { projection, snapshot, viewerUserId } = input;
  input.signal?.throwIfAborted();
  const shouldDefer = await shouldDeferForActiveEditor(input);
  input.signal?.throwIfAborted();
  if (shouldDefer) return "deferred";
  if (projection.source.documentId === null) {
    const existing = await loadSessionShareSyncState(
      viewerUserId,
      snapshot.shareId,
    );
    input.signal?.throwIfAborted();
    await writeSyncState(
      existing
        ? { ...existing, status: "conflict" }
        : {
            viewerUserId,
            shareId: snapshot.shareId,
            sessionId: snapshot.sessionId,
            acknowledgedContentRevision:
              snapshot.webEditBase?.contentRevision ?? snapshot.contentRevision,
            baselineSourceHash: snapshot.webEditBase
              ? await hashSessionShareProjection({
                  title: snapshot.webEditBase.title,
                  body: snapshot.webEditBase.body,
                })
              : projection.hash,
            status: "conflict",
          },
      input.signal,
    );
    return "conflict";
  }

  const localAttachments = await loadSessionShareAttachments(
    snapshot.sessionId,
  );
  input.signal?.throwIfAborted();
  const localToShared = matchSharedAttachmentsToLocal(
    localAttachments,
    snapshot.attachments,
  );
  const conflictState: SessionShareSyncState = input.priorState
    ? { ...input.priorState, status: "conflict" }
    : {
        viewerUserId,
        shareId: snapshot.shareId,
        sessionId: snapshot.sessionId,
        acknowledgedContentRevision:
          snapshot.webEditBase?.contentRevision ?? snapshot.contentRevision,
        baselineSourceHash: snapshot.webEditBase
          ? await hashSessionShareProjection({
              title: snapshot.webEditBase.title,
              body: snapshot.webEditBase.body,
            })
          : projection.hash,
        status: "conflict",
      };
  let localBody: JSONContent;
  try {
    localBody = restoreLocalAttachmentIds(
      snapshot.body,
      projection.source.body,
      localAttachments,
      localToShared,
    );
  } catch {
    const state = await loadSessionShareSyncState(
      viewerUserId,
      snapshot.shareId,
    );
    input.signal?.throwIfAborted();
    await writeSyncState(
      state ? { ...state, status: "conflict" } : conflictState,
      input.signal,
    );
    return "conflict";
  }
  const nextBody = JSON.stringify(localBody);
  const now = new Date().toISOString();
  const nextState: SessionShareSyncState = {
    viewerUserId,
    shareId: snapshot.shareId,
    sessionId: snapshot.sessionId,
    acknowledgedContentRevision: snapshot.contentRevision,
    baselineSourceHash: input.snapshotHash,
    status: "clean",
  };
  input.signal?.throwIfAborted();

  const outcome = await enqueueDatabaseWrite(
    `session:${snapshot.sessionId}`,
    async () => {
      input.signal?.throwIfAborted();
      const shouldDefer = await shouldDeferForActiveEditor(input);
      input.signal?.throwIfAborted();
      if (shouldDefer) return "deferred" as const;
      const releaseImportLock = input.acquireSessionImportLock?.(
        snapshot.sessionId,
      );
      if (input.acquireSessionImportLock && !releaseImportLock) {
        return "deferred" as const;
      }
      try {
        input.signal?.throwIfAborted();
        await executeTransaction([
          {
            sql: `
          UPDATE sessions
          SET title = ?, updated_at = ?
          WHERE id = ?
            AND title = ?
            AND deleted_at IS NULL
        `,
            params: [
              snapshot.title,
              now,
              snapshot.sessionId,
              projection.source.title,
            ],
            expectedRowsAffected: 1,
          },
          {
            sql: `
          UPDATE session_documents
          SET body = ?, body_format = 'prosemirror_json', updated_by = ?, updated_at = ?
          WHERE id = ?
            AND session_id = ?
            AND kind = 'note'
            AND body = ?
            AND body_format = ?
            AND deleted_at IS NULL
        `,
            params: [
              nextBody,
              viewerUserId,
              now,
              projection.source.documentId,
              snapshot.sessionId,
              projection.source.rawBody,
              projection.source.bodyFormat,
            ],
            expectedRowsAffected: 1,
          },
          syncStateStatement(nextState),
        ]);
        input.signal?.throwIfAborted();
        if (!releaseImportLock && (await shouldDeferForActiveEditor(input))) {
          input.signal?.throwIfAborted();
          await executeTransaction([syncStateStatement(conflictState)]);
          input.signal?.throwIfAborted();
          return "conflict" as const;
        }
        return "imported" as const;
      } finally {
        releaseImportLock?.();
      }
    },
  );
  input.signal?.throwIfAborted();

  if (outcome !== "imported") return outcome;

  await bestEffortAcknowledge(input);
  return "imported";
}

async function bestEffortAcknowledge(input: {
  snapshot: SharedNoteSnapshot;
  signal?: AbortSignal;
  acknowledge?: (shareId: string, contentRevision: number) => Promise<void>;
  isSessionEditorActive?: (sessionId: string) => boolean | Promise<boolean>;
}) {
  input.signal?.throwIfAborted();
  if (!input.snapshot.webEditBase || !input.acknowledge) return;
  const shouldDefer = await shouldDeferForActiveEditor(input);
  input.signal?.throwIfAborted();
  if (shouldDefer) return;
  try {
    await input.acknowledge(
      input.snapshot.shareId,
      input.snapshot.contentRevision,
    );
  } catch {
    input.signal?.throwIfAborted();
  }
  input.signal?.throwIfAborted();
}

async function shouldDeferForActiveEditor(input: {
  snapshot: SharedNoteSnapshot;
  isSessionEditorActive?: (sessionId: string) => boolean | Promise<boolean>;
}) {
  return Boolean(await input.isSessionEditorActive?.(input.snapshot.sessionId));
}

async function writeSyncState(
  state: SessionShareSyncState,
  signal?: AbortSignal,
): Promise<void> {
  signal?.throwIfAborted();
  await enqueueDatabaseWrite(`session:${state.sessionId}`, () => {
    signal?.throwIfAborted();
    return executeTransaction([syncStateStatement(state)]);
  });
  signal?.throwIfAborted();
}

function syncStateStatement(state: SessionShareSyncState) {
  return {
    sql: `
      INSERT INTO session_share_sync_state (
        viewer_user_id,
        share_id,
        session_id,
        acknowledged_content_revision,
        baseline_source_hash,
        status,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
      ON CONFLICT(viewer_user_id, share_id) DO UPDATE SET
        session_id = excluded.session_id,
        acknowledged_content_revision = excluded.acknowledged_content_revision,
        baseline_source_hash = excluded.baseline_source_hash,
        status = excluded.status,
        updated_at = excluded.updated_at
      WHERE session_share_sync_state.acknowledged_content_revision
        <= excluded.acknowledged_content_revision
    `,
    params: [
      state.viewerUserId,
      state.shareId,
      state.sessionId,
      state.acknowledgedContentRevision,
      state.baselineSourceHash,
      state.status,
    ],
    expectedRowsAffected: 1,
  };
}

function canonicalJson(value: unknown): string {
  return JSON.stringify(sortJson(value));
}

function sortJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortJson);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([, child]) => child !== undefined)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, child]) => [key, sortJson(child)]),
  );
}
