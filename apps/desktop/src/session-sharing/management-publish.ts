import {
  addSharedAttachmentIds,
  attachmentMetadataMatches,
  isAttachmentShareable,
  loadSessionShareAttachments,
  matchSharedAttachmentsToLocal,
} from "./attachments";
import {
  publishSessionShareSnapshot,
  type SessionShareManagement,
  ShareManagementError,
} from "./client";
import { flushCanonicalSessionEditorChanges } from "./editor-activity";
import type { SharePanelIdentity } from "./management";
import type {
  PublishLatestSessionShare,
  RequireActiveShareContext,
} from "./management-operation";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  loadSessionShareSyncState,
  recordPublishedSessionShareState,
} from "./reconciliation";
import { loadSessionShareSource } from "./source";

import { env } from "~/env";
import {
  type SharedNoteAttachment,
  type SharedNoteSnapshot,
  upsertDurableSharedNoteCache,
} from "~/shared-notes/cache";

export function createPublishLatestSessionShare({
  sessionId,
  identity,
  management,
  sharedAttachmentsReady,
  sharedSnapshot,
  sharedAttachments,
  requireActiveContext,
}: {
  sessionId: string;
  identity: SharePanelIdentity;
  management: SessionShareManagement | undefined;
  sharedAttachmentsReady: boolean;
  sharedSnapshot: SharedNoteSnapshot | null;
  sharedAttachments: SharedNoteAttachment[];
  requireActiveContext: RequireActiveShareContext;
}): PublishLatestSessionShare {
  return async (
    signal,
    requestedAttachments = sharedAttachments,
    localOverrides = new Map<string, string>(),
    resolveConflict = false,
  ) => {
    if (!management) throw new ShareManagementError();
    if (!sharedAttachmentsReady || !sharedSnapshot) {
      throw new ShareManagementError();
    }
    await flushCanonicalSessionEditorChanges(sessionId);
    requireActiveContext(signal);
    const syncState = await loadSessionShareSyncState(
      identity.ownerUserId,
      identity.shareId,
      sessionId,
    );
    const syncStateIsCurrent =
      syncState?.status === "clean" &&
      syncState.sessionId === sharedSnapshot.sessionId &&
      syncState.acknowledgedContentRevision === sharedSnapshot.contentRevision;
    const canResolveCurrentConflict = Boolean(
      resolveConflict &&
      syncState?.status === "conflict" &&
      syncState.sessionId === sharedSnapshot.sessionId &&
      syncState.acknowledgedContentRevision <= sharedSnapshot.contentRevision,
    );
    if (sharedSnapshot.webEditBase && !canResolveCurrentConflict) {
      throw new ShareManagementError();
    }
    if (!syncStateIsCurrent && !canResolveCurrentConflict) {
      throw new ShareManagementError();
    }
    const context = requireActiveContext(signal);
    const source = await loadSessionShareSource(
      sessionId,
      context.session.user.id,
    );
    if (
      source.sessionId !== management.sessionId ||
      source.workspaceId !== management.workspaceId
    ) {
      throw new ShareManagementError();
    }
    const activeContext = requireActiveContext(signal);
    const localAttachments = await loadSessionShareAttachments(sessionId);
    const localToShared = matchSharedAttachmentsToLocal(
      localAttachments,
      requestedAttachments,
    );
    for (const [localId, sharedId] of localOverrides) {
      const local = localAttachments.find(
        (attachment) => attachment.id === localId,
      );
      const shared = requestedAttachments.find(
        (attachment) => attachment.id === sharedId,
      );
      if (
        !local ||
        !shared ||
        !isAttachmentShareable(local) ||
        !attachmentMetadataMatches(local, shared)
      ) {
        throw new ShareManagementError();
      }
      localToShared.set(localId, sharedId);
    }
    const mappedIds = new Set(localToShared.values());
    const publishableAttachments = requestedAttachments.filter((attachment) =>
      mappedIds.has(attachment.id),
    );
    const body = addSharedAttachmentIds(
      source.body,
      localAttachments,
      localToShared,
    );
    const sourceHash = await hashSessionShareProjection({
      title: source.title,
      body,
    });
    const baseRevision = sharedSnapshot.contentRevision;
    const published = await publishSessionShareSnapshot({
      apiBaseUrl: env.VITE_API_URL,
      session: activeContext.session,
      shareId: identity.shareId,
      baseRevision,
      mutationId: await createSessionShareMutationId({
        shareId: identity.shareId,
        baseRevision,
        sourceHash,
        attachmentIds: publishableAttachments.map(
          (attachment) => attachment.id,
        ),
      }),
      title: source.title,
      body,
      attachmentIds: publishableAttachments.map((attachment) => attachment.id),
      signal,
    });
    requireActiveContext(signal);
    await recordPublishedSessionShareState({
      viewerUserId: identity.ownerUserId,
      shareId: identity.shareId,
      sessionId: source.sessionId,
      contentRevision: published.contentRevision,
      sourceHash,
    });
    await upsertDurableSharedNoteCache(identity.ownerUserId, {
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
    requireActiveContext(signal);
    return published;
  };
}
