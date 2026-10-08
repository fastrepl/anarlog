import { useQueries, useQuery } from "@tanstack/react-query";

import {
  createOrReuseSessionShare,
  publishSessionShareSnapshot,
} from "./client";
import type { ShareManagementContext } from "./client-contract";
import { applyDefaultMeetingShareAccess } from "./default-access";
import {
  type FolderDefaultAccessRule,
  folderDefaultAccessRule,
  type FolderShareCandidate,
  folderShareCandidateReady,
  loadFolderDefaultAccessAppliedSessionIds,
  markFolderDefaultAccessApplied,
  setFolderDefaultAccess,
  useFolderDefaultAccessRules,
} from "./folder-default-access";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  recordPublishedSessionShareState,
} from "./reconciliation";
import {
  type AvailableShareWorkspace,
  loadSessionShareSource,
  SESSION_SHARE_DOCUMENT_ID_SQL,
  useAvailableShareWorkspaces,
} from "./source";

import { useAITask } from "~/ai/contexts";
import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useLiveQuery } from "~/db";
import { env } from "~/env";
import { useFolderWorkspaces } from "~/session/queries";
import {
  loadManagedSharedNoteForSession,
  markSessionShareActivated,
  upsertDurableSharedNoteCache,
} from "~/shared-notes/cache";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

const SHARE_DEBOUNCE_MS = 1500;
const EMPTY_CANDIDATES: FolderShareCandidate[] = [];

type FolderShareCandidateSqlRow = {
  session_id: string;
  folder_path: string;
  created_at: string;
  document_id: string;
  document_updated_at: string;
  title: string;
  body: string;
};

export function FolderDefaultAccessRunner() {
  const { supabase, session } = useAuth();
  const ownerUserId =
    session && session.user.is_anonymous !== true ? session.user.id : null;
  const rules = useFolderDefaultAccessRules();
  const getTaskState = useAITask((state) => state.getState);
  const workspaces = useAvailableShareWorkspaces(ownerUserId);
  const legacyFolderWorkspaces = useFolderWorkspaces();
  const legacyFolders = Object.entries(legacyFolderWorkspaces).filter(
    ([path, workspace]) =>
      !folderDefaultAccessRule(rules, path) &&
      workspaces.some((candidate) => candidate.id === workspace.workspaceId),
  );

  useQuery({
    queryKey: [
      "folder-default-access-legacy",
      ownerUserId,
      legacyFolders.map(([path, workspace]) => [path, workspace.workspaceId]),
    ],
    enabled: Boolean(ownerUserId) && legacyFolders.length > 0,
    queryFn: async () => {
      for (const [path, workspace] of legacyFolders) {
        await setFolderDefaultAccess(path, {
          access: "workspace",
          workspaceId: workspace.workspaceId,
        });
      }
      return true;
    },
  });

  const folderPaths = rules.map((rule) => rule.folder_path);
  const { data: candidates = EMPTY_CANDIDATES } = useLiveQuery<
    FolderShareCandidateSqlRow,
    FolderShareCandidate[]
  >({
    sql: `
      SELECT
        session.id AS session_id,
        session.folder_path,
        session.created_at,
        share_document.id AS document_id,
        share_document.updated_at AS document_updated_at,
        session.title,
        share_document.body
      FROM sessions AS session
      JOIN session_documents AS share_document
        ON share_document.id = (${SESSION_SHARE_DOCUMENT_ID_SQL})
      WHERE session.deleted_at IS NULL
        AND session.owner_user_id = ?
        AND session.folder_path IN (${folderPaths.map(() => "?").join(", ")})
        AND share_document.body <> ''
        AND NOT EXISTS (
          SELECT 1 FROM shared_session_cache AS cache
          WHERE cache.viewer_user_id = ?
            AND cache.session_id = session.id
            AND cache.manage_access = 1
        )
      ORDER BY session.created_at, session.id
    `,
    params: [ownerUserId ?? "", ...folderPaths, ownerUserId ?? ""],
    enabled: Boolean(ownerUserId) && folderPaths.length > 0,
    mapRows: (rows) =>
      rows.map((row) => ({
        sessionId: row.session_id,
        folderPath: row.folder_path,
        createdAt: row.created_at,
        documentId: row.document_id,
        documentUpdatedAt: row.document_updated_at,
        title: row.title,
        body: row.body,
      })),
  });

  useQueries({
    queries: candidates.flatMap((candidate) => {
      const rule = folderDefaultAccessRule(rules, candidate.folderPath);
      if (
        !supabase ||
        !session ||
        !ownerUserId ||
        !rule ||
        !folderShareCandidateReady(
          candidate,
          rule,
          getTaskState(createTaskId(candidate.documentId, "enhance"))
            ?.status === "generating",
        )
      ) {
        return [];
      }
      const ruleWorkspaces =
        rule.access === "workspace"
          ? workspaces.filter((workspace) => workspace.id === rule.workspace_id)
          : workspaces;
      if (rule.access === "workspace" && ruleWorkspaces.length === 0) {
        return [];
      }
      return [
        {
          queryKey: [
            "folder-default-access-share",
            ownerUserId,
            candidate.sessionId,
            rule.access,
            rule.workspace_id,
            candidate.documentUpdatedAt,
          ],
          retry: false,
          staleTime: Infinity,
          queryFn: async ({ signal }: { signal: AbortSignal }) => {
            await abortableDelay(SHARE_DEBOUNCE_MS, signal);
            await shareWithFolderDefaultAccess({
              context: { supabase, session, signal },
              ownerUserId,
              sessionId: candidate.sessionId,
              rule,
              workspaces: ruleWorkspaces,
              signal,
            });
            return true;
          },
        },
      ];
    }),
  });

  return null;
}

async function shareWithFolderDefaultAccess({
  context,
  ownerUserId,
  sessionId,
  rule,
  workspaces,
  signal,
}: {
  context: ShareManagementContext;
  ownerUserId: string;
  sessionId: string;
  rule: FolderDefaultAccessRule;
  workspaces: AvailableShareWorkspace[];
  signal: AbortSignal;
}): Promise<void> {
  const applied = await loadFolderDefaultAccessAppliedSessionIds();
  if (applied.has(sessionId)) return;
  if (await loadManagedSharedNoteForSession(ownerUserId, sessionId)) {
    await markFolderDefaultAccessApplied(sessionId);
    return;
  }
  signal.throwIfAborted();

  const source = await loadSessionShareSource(sessionId, ownerUserId);
  signal.throwIfAborted();
  const share = await createOrReuseSessionShare(context, {
    workspaceId: source.workspaceId,
    sessionId: source.sessionId,
  });
  if (!share.wasCreated) {
    await markFolderDefaultAccessApplied(sessionId);
    return;
  }
  trackAnalyticsEvent("share_created", { entry_point: "folder_default" });

  const sourceHash = await hashSessionShareProjection({
    title: source.title,
    body: source.body,
  });
  const published = await publishSessionShareSnapshot({
    apiBaseUrl: env.VITE_API_URL,
    session: context.session,
    shareId: share.shareId,
    baseRevision: 0,
    mutationId: await createSessionShareMutationId({
      shareId: share.shareId,
      baseRevision: 0,
      sourceHash,
      attachmentIds: [],
      participants: source.participants,
      meetingAt: source.meetingAt,
    }),
    title: source.title,
    body: source.body,
    participants: source.participants,
    meetingAt: source.meetingAt,
    attachmentIds: [],
    signal,
  });
  await recordPublishedSessionShareState({
    viewerUserId: ownerUserId,
    shareId: published.shareId,
    sessionId: source.sessionId,
    contentRevision: published.contentRevision,
    sourceHash,
  });
  await upsertDurableSharedNoteCache(ownerUserId, {
    shareId: published.shareId,
    workspaceId: source.workspaceId,
    sessionId: source.sessionId,
    schemaVersion: published.schemaVersion,
    contentRevision: published.contentRevision,
    title: published.title,
    body: published.body,
    attachments: published.attachments,
    capability: "editor",
    manageAccess: true,
    accessVersion: published.accessVersion,
    webEditable: published.webEditable,
    webEditBase: null,
    publishedAt: published.publishedAt,
  });
  await markFolderDefaultAccessApplied(sessionId);
  await applyDefaultMeetingShareAccess({
    wasCreated: true,
    actionType: "auto",
    access: rule.access,
    workspaces,
    context,
    shareId: share.shareId,
    sessionId: source.sessionId,
    noteTitle: source.title,
    signal,
    requireActive: () => signal.throwIfAborted(),
  });
  await markSessionShareActivated(ownerUserId, share.shareId, sessionId);
}

function abortableDelay(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(signal.reason);
      return;
    }
    const timeout = setTimeout(resolve, ms);
    signal.addEventListener(
      "abort",
      () => {
        clearTimeout(timeout);
        reject(signal.reason);
      },
      { once: true },
    );
  });
}
