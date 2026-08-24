import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { ShareNetwork, Users } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import {
  createOrReuseSessionShare,
  getSessionShareManagement,
  publishSessionShareSnapshot,
  setSessionShareScope,
  ShareManagementError,
} from "./client";
import {
  deliverSessionShareRecapEmail,
  deliverSessionShareRecapToSlack,
} from "./delivery-management";
import { type DraftShareAction, SessionShareDraftContent } from "./draft-panel";
import { flushCanonicalSessionEditorChanges } from "./editor-activity";
import { generalAccessWorkspaceId } from "./general-access";
import {
  deliverSessionShareInvitations,
  getSessionShareSenderName,
  reportSessionShareInvitations,
  type SessionShareInvitationDelivery,
} from "./invitation-management";
import {
  copySessionShareUrl,
  enableAndCopySessionShareLink,
  loadSharePanel,
  requireManagementContext,
  sessionShareManagementQueryKey,
  ShareOperationAbortedError,
  type SharePanelIdentity,
  type SharePreparationIdentity,
  withoutSignal,
} from "./management";
import { SessionSharePopoverContent } from "./management-panel";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  recordPublishedSessionShareState,
} from "./reconciliation";
import { loadSessionShareSource, useAvailableShareWorkspaces } from "./source";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { env } from "~/env";
import {
  loadManagedSharedNoteForSession,
  markSessionShareActivated,
  upsertDurableSharedNoteCache,
  useDurableSharedNote,
  useManagedDurableSharedNote,
} from "~/shared-notes/cache";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export { sessionShareManagementQueryKey };

export function SessionShareButton({ sessionId }: { sessionId: string }) {
  const auth = useAuth();
  const latestAuthRef = useRef(auth);
  latestAuthRef.current = auth;
  const latestSessionIdRef = useRef(sessionId);
  latestSessionIdRef.current = sessionId;
  const prepareControllersRef = useRef(new Set<AbortController>());
  const sharePreparationIdentityRef = useRef<SharePreparationIdentity | null>(
    null,
  );
  const nextSharePreparationAttemptRef = useRef(0);
  const cancelSharePreparation = () => {
    for (const controller of prepareControllersRef.current) {
      controller.abort();
    }
    prepareControllersRef.current.clear();
  };
  useMountEffect(() => () => {
    cancelSharePreparation();
  });
  const isActiveSharePreparation = (identity: SharePreparationIdentity) => {
    const active = sharePreparationIdentityRef.current;
    return (
      active?.ownerUserId === identity.ownerUserId &&
      active.sessionId === identity.sessionId &&
      active.attemptId === identity.attemptId
    );
  };
  const runPrepareOperation = async <T,>(
    operation: (signal: AbortSignal) => Promise<T>,
  ): Promise<T> => {
    const controller = new AbortController();
    prepareControllersRef.current.add(controller);
    try {
      const result = await operation(controller.signal);
      if (controller.signal.aborted) throw new ShareOperationAbortedError();
      return result;
    } catch (error) {
      if (controller.signal.aborted) {
        throw new ShareOperationAbortedError();
      }
      throw error;
    } finally {
      prepareControllersRef.current.delete(controller);
    }
  };
  const requireActivePrepareContext = (
    identity: SharePreparationIdentity,
    signal: AbortSignal,
  ) => {
    if (signal.aborted) throw new ShareManagementError();
    const context = requireManagementContext(latestAuthRef.current);
    if (
      context.session.user.id !== identity.ownerUserId ||
      latestSessionIdRef.current !== identity.sessionId
    ) {
      throw new ShareManagementError();
    }
    return { ...context, signal };
  };
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const [sharePanelIdentity, setSharePanelIdentity] =
    useState<SharePanelIdentity | null>(null);
  const sharePanelIdentityRef = useRef<SharePanelIdentity | null>(null);
  const [sharePreparationIdentity, setSharePreparationIdentity] =
    useState<SharePreparationIdentity | null>(null);
  const sharePanelPendingRef = useRef(false);
  const clearAbandonedSharePreparation = (
    identity: SharePreparationIdentity,
  ) => {
    if (isActiveSharePreparation(identity)) {
      sharePreparationIdentityRef.current = null;
    }
    setSharePreparationIdentity((current) =>
      current &&
      current.ownerUserId === identity.ownerUserId &&
      current.sessionId === identity.sessionId &&
      current.attemptId === identity.attemptId
        ? null
        : current,
    );
  };
  const accountUserId = auth.session?.user.id ?? null;
  const shareContextRef = useRef({ accountUserId, sessionId });
  if (
    shareContextRef.current.accountUserId !== accountUserId ||
    shareContextRef.current.sessionId !== sessionId
  ) {
    sharePreparationIdentityRef.current = null;
    sharePanelIdentityRef.current = null;
    shareContextRef.current = { accountUserId, sessionId };
  }
  const managedNoteQuery = useManagedDurableSharedNote(
    accountUserId,
    sessionId,
  );
  const workspaces = useAvailableShareWorkspaces(accountUserId);
  const activeSharePreparationIdentity =
    sharePreparationIdentity?.ownerUserId === accountUserId &&
    sharePreparationIdentity.sessionId === sessionId &&
    isActiveSharePreparation(sharePreparationIdentity)
      ? sharePreparationIdentity
      : null;
  const activationMutation = useMutation({
    mutationFn: ({
      identity,
      action,
    }: {
      identity: SharePreparationIdentity;
      action: DraftShareAction;
    }) =>
      runPrepareOperation(async (signal) => {
        let context = requireActivePrepareContext(identity, signal);
        await flushCanonicalSessionEditorChanges(identity.sessionId);
        context = requireActivePrepareContext(identity, signal);
        const source = await loadSessionShareSource(
          identity.sessionId,
          identity.ownerUserId,
        );
        context = requireActivePrepareContext(identity, signal);
        if (source.sessionId !== identity.sessionId) {
          throw new ShareManagementError();
        }
        const share = await createOrReuseSessionShare(context, {
          workspaceId: source.workspaceId,
          sessionId: source.sessionId,
        });
        if (share.wasCreated) {
          trackAnalyticsEvent("share_created", {
            entry_point: "session_header",
          });
        }
        context = requireActivePrepareContext(identity, signal);
        const management = await getSessionShareManagement(
          context,
          share.shareId,
        );
        if (
          management.workspaceId !== source.workspaceId ||
          management.sessionId !== source.sessionId
        ) {
          throw new ShareManagementError();
        }
        const cachedManagedShare = await loadManagedSharedNoteForSession(
          identity.ownerUserId,
          source.sessionId,
        );
        context = requireActivePrepareContext(identity, signal);
        if (
          cachedManagedShare &&
          (cachedManagedShare.shareId !== share.shareId ||
            cachedManagedShare.workspaceId !== source.workspaceId ||
            cachedManagedShare.sessionId !== source.sessionId)
        ) {
          throw new ShareManagementError();
        }
        if (share.wasCreated || !cachedManagedShare) {
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
          context = requireActivePrepareContext(identity, signal);
          await recordPublishedSessionShareState({
            viewerUserId: identity.ownerUserId,
            shareId: published.shareId,
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
          context = requireActivePrepareContext(identity, signal);
        }
        let actionResult:
          | { type: "invite"; deliveries: SessionShareInvitationDelivery[] }
          | { type: "email"; recipientCount: number }
          | { type: "slack"; channelName: string }
          | { type: "copy-link" }
          | { type: "scope"; copied: boolean };
        if (action.type === "invite") {
          const deliveries = await deliverSessionShareInvitations({
            context,
            shareId: share.shareId,
            emails: action.emails,
            capability: "viewer",
            noteTitle: source.title,
            senderName: getSessionShareSenderName(context.session.user),
            signal,
            requireActive: () => {
              requireActivePrepareContext(identity, signal);
            },
          });
          if (!deliveries.some((delivery) => delivery.deliveredBy)) {
            throw new ShareManagementError();
          }
          actionResult = {
            type: "invite",
            deliveries,
          };
        } else if (action.type === "email") {
          await deliverSessionShareRecapEmail({
            context,
            shareId: share.shareId,
            recipients: action.emails,
            noteTitle: source.title,
            body: source.body,
            signal,
          });
          actionResult = {
            type: "email",
            recipientCount: action.emails.length,
          };
        } else if (action.type === "slack") {
          await deliverSessionShareRecapToSlack({
            context,
            channel: action.channel,
            noteTitle: source.title,
            body: source.body,
            signal,
          });
          actionResult = {
            type: "slack",
            channelName: action.channel.name,
          };
        } else if (action.type === "copy-link") {
          await copySessionShareUrl(context, share.shareId, () =>
            requireActivePrepareContext(identity, signal),
          );
          actionResult = { type: "copy-link" };
        } else if (action.target === "restricted") {
          await setSessionShareScope(context, {
            shareId: share.shareId,
            scope: "restricted",
          });
          actionResult = { type: "scope", copied: false };
        } else if (action.target === "link") {
          await enableAndCopySessionShareLink({
            context,
            shareId: share.shareId,
            hasActiveLink: management.hasActiveLink,
            assertActive: () => requireActivePrepareContext(identity, signal),
          });
          actionResult = { type: "scope", copied: true };
        } else {
          const workspaceId = generalAccessWorkspaceId(
            action.target,
            workspaces,
          );
          if (!workspaceId) throw new ShareManagementError();
          try {
            await setSessionShareScope(context, {
              shareId: share.shareId,
              scope: "workspace",
              workspaceId,
            });
            requireActivePrepareContext(identity, signal);
          } catch {
            await setSessionShareScope(withoutSignal(context), {
              shareId: share.shareId,
              scope: "restricted",
            }).catch(() => undefined);
            throw new ShareManagementError();
          }
          actionResult = { type: "scope", copied: false };
        }
        requireActivePrepareContext(identity, signal);
        await markSessionShareActivated(
          identity.ownerUserId,
          share.shareId,
          identity.sessionId,
        );
        context = requireActivePrepareContext(identity, signal);
        const data = await loadSharePanel(context, share.shareId);
        requireActivePrepareContext(identity, signal);
        return {
          identity: { ...identity, shareId: share.shareId },
          data: { ...data, wasCreated: share.wasCreated },
          actionResult,
        };
      }),
    onSuccess: ({ identity, data, actionResult }) => {
      if (
        !isActiveSharePreparation(identity) ||
        latestAuthRef.current.session?.user.id !== identity.ownerUserId ||
        latestSessionIdRef.current !== identity.sessionId
      ) {
        clearAbandonedSharePreparation(identity);
        return;
      }
      queryClient.setQueryData(
        sessionShareManagementQueryKey(identity.ownerUserId, identity.shareId),
        data,
      );
      void queryClient.invalidateQueries({
        queryKey: ["durable-shared-note-cache", identity.ownerUserId],
      });
      if (actionResult.type === "invite") {
        reportSessionShareInvitations(actionResult.deliveries, "viewer");
      } else if (actionResult.type === "email") {
        trackAnalyticsEvent("share_recap_sent", {
          delivery_method: "email",
          recipient_count: actionResult.recipientCount,
        });
        sonnerToast.success(
          actionResult.recipientCount > 1
            ? t`Meeting notes sent.`
            : t`Meeting note sent.`,
        );
      } else if (actionResult.type === "slack") {
        trackAnalyticsEvent("share_recap_sent", {
          delivery_method: "slack",
        });
        sonnerToast.success(
          t`Meeting notes sent to #${actionResult.channelName}.`,
        );
      } else if (actionResult.type === "copy-link") {
        trackAnalyticsEvent("share_link_copied", {
          entry_point: "share_panel",
        });
        sonnerToast.success(t`Share link copied.`);
      } else {
        sonnerToast.success(
          actionResult.copied
            ? t`Anyone with the link can view. Link copied.`
            : t`Access updated.`,
        );
      }
      sharePreparationIdentityRef.current = null;
      setSharePreparationIdentity(null);
      sharePanelIdentityRef.current = identity;
      setSharePanelIdentity(identity);
    },
    onError: (error, variables) => {
      if (
        error instanceof ShareOperationAbortedError ||
        !isActiveSharePreparation(variables.identity) ||
        latestAuthRef.current.session?.user.id !==
          variables.identity.ownerUserId ||
        latestSessionIdRef.current !== variables.identity.sessionId
      ) {
        clearAbandonedSharePreparation(variables.identity);
        return;
      }
      console.error("[session-sharing] could not activate share", error);
      sonnerToast.error(
        variables.action.type === "invite"
          ? variables.action.emails.length > 1
            ? "Could not create these invitations."
            : "Could not create this invitation."
          : variables.action.type === "email"
            ? "Could not email the meeting notes."
            : variables.action.type === "slack"
              ? "Could not send the meeting notes to Slack."
              : variables.action.type === "scope"
                ? "Could not update general access."
                : "Could not copy the share link.",
      );
    },
  });
  const cachedSharePanelIdentity =
    activeSharePreparationIdentity &&
    managedNoteQuery.data &&
    !activationMutation.isPending &&
    !activationMutation.isError
      ? {
          ...activeSharePreparationIdentity,
          shareId: managedNoteQuery.data.shareId,
        }
      : null;
  const activeSharePanelIdentity =
    sharePanelIdentity?.ownerUserId === accountUserId &&
    sharePanelIdentity.sessionId === sessionId &&
    sharePanelIdentityRef.current === sharePanelIdentity
      ? sharePanelIdentity
      : cachedSharePanelIdentity;
  const showUpgradePrompt = Boolean(
    activeSharePreparationIdentity &&
    !activeSharePanelIdentity &&
    !managedNoteQuery.isLoading &&
    billing.isReady &&
    !billing.isPaid,
  );
  const sharePopoverOpen = Boolean(
    activeSharePanelIdentity || activeSharePreparationIdentity,
  );
  const durableNoteQuery = useDurableSharedNote(
    accountUserId,
    activeSharePanelIdentity?.shareId ?? "",
  );

  const queryKey = sessionShareManagementQueryKey(
    activeSharePanelIdentity?.ownerUserId ?? "",
    activeSharePanelIdentity?.shareId ?? "",
  );
  const shareQuery = useQuery({
    queryKey,
    enabled: Boolean(activeSharePanelIdentity),
    queryFn: async ({ signal }) => {
      const context = requireManagementContext(auth);
      if (context.session.user.id !== activeSharePanelIdentity?.ownerUserId) {
        throw new ShareManagementError();
      }
      return loadSharePanel(
        { ...context, signal },
        activeSharePanelIdentity.shareId,
      );
    },
  });
  const sharedAttachments = durableNoteQuery.data?.attachments ?? [];
  const sharedAttachmentsReady = Boolean(
    activeSharePanelIdentity &&
    !durableNoteQuery.isLoading &&
    durableNoteQuery.data,
  );

  const closeSharePopover = () => {
    sharePreparationIdentityRef.current = null;
    sharePanelIdentityRef.current = null;
    cancelSharePreparation();
    setSharePanelIdentity(null);
    setSharePreparationIdentity(null);
    activationMutation.reset();
  };

  const openSharePopover = (
    identity: Omit<SharePreparationIdentity, "attemptId">,
  ) => {
    cancelSharePreparation();
    activationMutation.reset();
    const preparation = {
      ...identity,
      attemptId: nextSharePreparationAttemptRef.current,
    };
    nextSharePreparationAttemptRef.current += 1;
    sharePreparationIdentityRef.current = preparation;
    setSharePreparationIdentity(preparation);
  };

  const handleDraftAction = (action: DraftShareAction) => {
    if (
      !activeSharePreparationIdentity ||
      !isActiveSharePreparation(activeSharePreparationIdentity) ||
      managedNoteQuery.isLoading ||
      !billing.isReady ||
      !billing.isPaid
    ) {
      return;
    }
    activationMutation.reset();
    activationMutation.mutate({
      identity: activeSharePreparationIdentity,
      action,
    });
  };

  const handleShare = () => {
    if (sharePopoverOpen) {
      closeSharePopover();
      return;
    }
    if (!auth.session || auth.session.user.is_anonymous === true) {
      void auth.signIn().catch(() => {
        sonnerToast.error(t`Could not start sign-in.`);
      });
      return;
    }
    openSharePopover({
      ownerUserId: auth.session.user.id,
      sessionId,
    });
  };

  return (
    <Popover
      open={sharePopoverOpen}
      onOpenChange={(open) => {
        if (!open) closeSharePopover();
      }}
    >
      <PopoverTrigger asChild>
        <Button
          key={accountUserId ?? "signed-out"}
          type="button"
          size="icon"
          variant="ghost"
          data-tauri-drag-region="false"
          aria-label={t`Share note`}
          aria-expanded={sharePopoverOpen}
          title={t`Share note`}
          onClick={handleShare}
          className={cn([
            "text-muted-foreground hover:text-foreground rounded-full",
            sharePopoverOpen && "bg-accent text-foreground",
          ])}
        >
          <ShareNetwork className="size-4" aria-hidden="true" />
        </Button>
      </PopoverTrigger>
      {showUpgradePrompt ? (
        <SessionShareUpgradeContent onUpgrade={billing.upgradeToPro} />
      ) : activeSharePanelIdentity ? (
        <SessionSharePopoverContent
          key={`${activeSharePanelIdentity.ownerUserId}:${activeSharePanelIdentity.shareId}:${activeSharePanelIdentity.sessionId}`}
          sessionId={activeSharePanelIdentity.sessionId}
          identity={activeSharePanelIdentity}
          data={shareQuery.data}
          error={shareQuery.isError}
          canExpand={billing.isPaid}
          sharedAttachments={sharedAttachments}
          sharedSnapshot={durableNoteQuery.data ?? null}
          sharedAttachmentsReady={sharedAttachmentsReady}
          workspaces={workspaces}
          pendingRef={sharePanelPendingRef}
          onRetry={() => void shareQuery.refetch()}
          onActivated={() =>
            markSessionShareActivated(
              activeSharePanelIdentity.ownerUserId,
              activeSharePanelIdentity.shareId,
              activeSharePanelIdentity.sessionId,
            )
          }
          onChanged={() =>
            Promise.all([
              queryClient.invalidateQueries({ queryKey }),
              queryClient.invalidateQueries({
                queryKey: [
                  "durable-shared-note-cache",
                  activeSharePanelIdentity.ownerUserId,
                ],
              }),
            ])
          }
        />
      ) : activeSharePreparationIdentity ? (
        <SessionShareDraftContent
          sessionId={activeSharePreparationIdentity.sessionId}
          disabled={
            managedNoteQuery.isLoading || !billing.isReady || !billing.isPaid
          }
          pendingAction={
            activationMutation.isPending
              ? (activationMutation.variables?.action ?? null)
              : null
          }
          workspaces={workspaces}
          onAction={handleDraftAction}
        />
      ) : null}
    </Popover>
  );
}

function SessionShareUpgradeContent({ onUpgrade }: { onUpgrade: () => void }) {
  useMountEffect(() => {
    trackAnalyticsEvent("paywall_viewed", {
      entry_point: "session_sharing",
      feature: "sharing",
    });
  });

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-upgrade-heading"
      aria-describedby="session-share-upgrade-description"
      className="w-[440px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="flex max-h-[min(530px,calc(100vh-74px))] flex-col items-center overflow-y-auto px-6 py-7 text-center">
        <div className="bg-accent flex size-10 items-center justify-center rounded-full">
          <Users className="size-4" aria-hidden="true" />
        </div>
        <h2
          id="session-share-upgrade-heading"
          className="mt-3 text-sm font-semibold"
        >
          <Trans>Share notes with others</Trans>
        </h2>
        <p
          id="session-share-upgrade-description"
          className="text-muted-foreground mt-1 text-xs leading-5"
        >
          <Trans>
            Upgrade to Pro to invite people and share this note with them.
          </Trans>
        </p>
        <Button type="button" size="sm" onClick={onUpgrade} className="mt-4">
          <Trans>Upgrade to Pro</Trans>
        </Button>
      </AppFloatingPanel>
    </PopoverContent>
  );
}
