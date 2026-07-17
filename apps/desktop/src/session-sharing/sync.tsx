import { useQueries } from "@tanstack/react-query";

import {
  addSharedAttachmentIds,
  loadSessionShareAttachments,
  matchSharedAttachmentsToLocal,
} from "./attachments";
import { publishSessionShareSnapshot } from "./client";
import { loadSessionShareSource } from "./source";

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
};

type OwnedShareSourceRevisionSqlRow = {
  share_id: string;
  workspace_id: string;
  session_id: string;
  source_updated_at: string;
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
        END AS source_updated_at
      FROM shared_session_cache AS cache
      JOIN sessions AS session
        ON session.id = cache.session_id
        AND session.workspace_id = cache.workspace_id
        AND session.deleted_at IS NULL
      LEFT JOIN session_documents AS note
        ON note.id = session.id
        AND note.session_id = session.id
        AND note.kind = 'note'
        AND note.deleted_at IS NULL
      WHERE cache.viewer_user_id = ?
        AND cache.manage_access = 1
      ORDER BY cache.share_id
    `,
    params: [ownerUserId ?? ""],
    enabled: Boolean(ownerUserId && session?.user.is_anonymous !== true),
    mapRows: (rows) =>
      rows.map((row) => ({
        shareId: row.share_id,
        workspaceId: row.workspace_id,
        sessionId: row.session_id,
        sourceUpdatedAt: row.source_updated_at,
      })),
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
        !shouldPublishOwnedShare(revision.sourceUpdatedAt, durable.publishedAt)
      ) {
        return [];
      }
      return [
        {
          queryKey: [
            "owned-shared-note-publish",
            ownerUserId,
            revision.shareId,
            revision.sourceUpdatedAt,
          ],
          queryFn: async ({ signal }: { signal: AbortSignal }) => {
            await abortableDelay(PUBLISH_DEBOUNCE_MS, signal);
            const source = await loadSessionShareSource(
              revision.sessionId,
              ownerUserId,
            );
            signal.throwIfAborted();
            if (
              source.workspaceId !== revision.workspaceId ||
              source.sessionId !== revision.sessionId
            ) {
              throw new Error("Shared note source changed");
            }
            const localAttachments = await loadSessionShareAttachments(
              revision.sessionId,
            );
            const localToShared = matchSharedAttachmentsToLocal(
              localAttachments,
              durable.attachments,
            );
            const mappedIds = new Set(localToShared.values());
            if (
              durable.attachments.some(
                (attachment) => !mappedIds.has(attachment.id),
              )
            ) {
              throw new Error("Shared attachment metadata is not available");
            }
            const published = await publishSessionShareSnapshot({
              apiBaseUrl: env.VITE_API_URL,
              session,
              shareId: revision.shareId,
              title: source.title,
              body: addSharedAttachmentIds(
                source.body,
                localAttachments,
                localToShared,
              ),
              attachmentIds: durable.attachments.map(
                (attachment) => attachment.id,
              ),
              signal,
            });
            signal.throwIfAborted();
            await upsertDurableSharedNoteCache(ownerUserId, {
              ...durable,
              schemaVersion: published.schemaVersion,
              contentRevision: published.contentRevision,
              title: published.title,
              body: published.body,
              attachments: published.attachments,
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

export function shouldPublishOwnedShare(
  sourceUpdatedAt: string,
  publishedAt: string,
) {
  const sourceTime = Date.parse(sourceUpdatedAt);
  const publishedTime = Date.parse(publishedAt);
  return (
    Number.isFinite(sourceTime) &&
    Number.isFinite(publishedTime) &&
    sourceTime > publishedTime
  );
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
