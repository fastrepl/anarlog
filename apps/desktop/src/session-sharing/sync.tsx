import { useQueries } from "@tanstack/react-query";

import { publishSessionShareSnapshot } from "./client";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  loadManagedShareProjection,
  loadSessionShareSyncState,
  recordPublishedSessionShareState,
} from "./reconciliation";

import { useAuth } from "~/auth";
import { useLiveQuery } from "~/db";
import { env } from "~/env";
import {
  upsertDurableSharedNoteCache,
  useDurableSharedNotes,
} from "~/shared-notes/cache";

const PUBLISH_DEBOUNCE_MS = 800;
const EMPTY_SOURCES: OwnedShareSourceRevision[] = [];

type OwnedShareSourceRevision = {
  shareId: string;
  workspaceId: string;
  sessionId: string;
  sourceUpdatedAt: string;
  acknowledgedContentRevision: number | null;
  baselineSourceHash: string | null;
  syncStatus: "clean" | "conflict" | null;
};

type OwnedShareSourceRevisionSqlRow = {
  share_id: string;
  workspace_id: string;
  session_id: string;
  source_updated_at: string;
  acknowledged_content_revision: number | null;
  baseline_source_hash: string | null;
  sync_status: string | null;
};

export function OwnedSharedNotePublisher() {
  const { session } = useAuth();
  const ownerUserId = session?.user.id ?? null;
  const durableNotes = useDurableSharedNotes(ownerUserId);
  const { data: sourceRevisions = EMPTY_SOURCES } = useLiveQuery<
    OwnedShareSourceRevisionSqlRow,
    OwnedShareSourceRevision[]
  >({
    sql: `
      SELECT
        cache.share_id,
        cache.workspace_id,
        cache.session_id,
        CASE
          WHEN note.updated_at > session.updated_at THEN note.updated_at
          ELSE session.updated_at
        END AS source_updated_at,
        sync.acknowledged_content_revision,
        sync.baseline_source_hash,
        sync.status AS sync_status
      FROM shared_session_cache AS cache
      JOIN sessions AS session
        ON session.id = cache.session_id
        AND session.deleted_at IS NULL
      JOIN session_documents AS note
        ON note.id = COALESCE(
          (
            SELECT canonical.id
            FROM session_documents AS canonical
            WHERE canonical.id = session.id
              AND canonical.session_id = session.id
              AND canonical.kind = 'note'
              AND canonical.deleted_at IS NULL
            LIMIT 1
          ),
          (
            SELECT fallback.id
            FROM session_documents AS fallback
            WHERE fallback.session_id = session.id
              AND fallback.kind = 'note'
              AND fallback.deleted_at IS NULL
            ORDER BY fallback.updated_at DESC, fallback.created_at DESC, fallback.id
            LIMIT 1
          )
        )
      LEFT JOIN session_share_sync_state AS sync
        ON sync.viewer_user_id = cache.viewer_user_id
        AND sync.share_id = cache.share_id
        AND sync.session_id = cache.session_id
      WHERE cache.viewer_user_id = ?
        AND cache.manage_access = 1
      ORDER BY cache.share_id
    `,
    params: [ownerUserId ?? ""],
    enabled: Boolean(ownerUserId && session?.user.is_anonymous !== true),
    mapRows: (rows) => rows.map(parseSourceRevision),
  });
  const durableByShareId = new Map(
    durableNotes
      .filter((note) => note.manageAccess)
      .map((note) => [note.shareId, note] as const),
  );

  useQueries({
    queries: sourceRevisions.flatMap((revision) => {
      const durable = durableByShareId.get(revision.shareId);
      if (
        !session ||
        session.user.is_anonymous === true ||
        !ownerUserId ||
        !durable ||
        durable.webEditBase ||
        revision.syncStatus === "conflict" ||
        (revision.acknowledgedContentRevision === null &&
          durable.webEditable) ||
        (revision.acknowledgedContentRevision !== null &&
          revision.acknowledgedContentRevision !== durable.contentRevision)
      ) {
        return [];
      }
      return [
        {
          queryKey: [
            "owned-shared-note-publish",
            ownerUserId,
            revision.shareId,
            durable.contentRevision,
            revision.sourceUpdatedAt,
            revision.baselineSourceHash,
          ],
          queryFn: async ({ signal }: { signal: AbortSignal }) => {
            await abortableDelay(PUBLISH_DEBOUNCE_MS, signal);
            const projection = await loadManagedShareProjection(
              ownerUserId,
              durable,
            );
            signal.throwIfAborted();
            const currentState = await loadSessionShareSyncState(
              ownerUserId,
              revision.shareId,
            );
            signal.throwIfAborted();
            if (durable.webEditBase || currentState?.status === "conflict") {
              return durable.contentRevision;
            }
            if (
              currentState &&
              (currentState.acknowledgedContentRevision !==
                durable.contentRevision ||
                currentState.baselineSourceHash === projection.hash)
            ) {
              return durable.contentRevision;
            }
            if (!currentState) {
              if (durable.webEditable) return durable.contentRevision;
              const durableHash = await hashSessionShareProjection({
                title: durable.title,
                body: durable.body,
              });
              signal.throwIfAborted();
              if (durableHash !== projection.hash) {
                return durable.contentRevision;
              }
            }

            const mutationId = await createSessionShareMutationId({
              shareId: revision.shareId,
              baseRevision: durable.contentRevision,
              sourceHash: projection.hash,
              attachmentIds: durable.attachments.map(
                (attachment) => attachment.id,
              ),
            });
            const published = await publishSessionShareSnapshot({
              apiBaseUrl: env.VITE_API_URL,
              session,
              shareId: revision.shareId,
              baseRevision: durable.contentRevision,
              mutationId,
              title: projection.source.title,
              body: projection.body,
              attachmentIds: durable.attachments.map(
                (attachment) => attachment.id,
              ),
              signal,
            });
            signal.throwIfAborted();
            await recordPublishedSessionShareState({
              viewerUserId: ownerUserId,
              shareId: revision.shareId,
              sessionId: revision.sessionId,
              contentRevision: published.contentRevision,
              sourceHash: projection.hash,
            });
            signal.throwIfAborted();
            await upsertDurableSharedNoteCache(ownerUserId, {
              ...durable,
              schemaVersion: published.schemaVersion,
              contentRevision: published.contentRevision,
              title: published.title,
              body: published.body,
              attachments: published.attachments,
              accessVersion: published.accessVersion,
              webEditable: published.webEditable,
              webEditBase: null,
              publishedAt: published.publishedAt,
            });
            return published.contentRevision;
          },
          staleTime: Infinity,
          retry: false,
        },
      ];
    }),
  });

  return null;
}

function parseSourceRevision(
  row: OwnedShareSourceRevisionSqlRow,
): OwnedShareSourceRevision {
  const status = row.sync_status;
  if (status !== null && status !== "clean" && status !== "conflict") {
    throw new Error("Invalid shared-note sync status");
  }
  return {
    shareId: row.share_id,
    workspaceId: row.workspace_id,
    sessionId: row.session_id,
    sourceUpdatedAt: row.source_updated_at,
    acknowledgedContentRevision: row.acknowledged_content_revision,
    baselineSourceHash: row.baseline_source_hash,
    syncStatus: status,
  };
}

function abortableDelay(durationMs: number, signal: AbortSignal) {
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) {
      reject(signal.reason);
      return;
    }
    const handleAbort = () => {
      clearTimeout(timeout);
      reject(signal.reason);
    };
    const timeout = setTimeout(() => {
      signal.removeEventListener("abort", handleAbort);
      resolve();
    }, durationMs);
    signal.addEventListener("abort", handleAbort, { once: true });
  });
}
