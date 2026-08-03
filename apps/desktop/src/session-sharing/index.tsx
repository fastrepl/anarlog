import { Trans } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  Check,
  CircleNotch,
  ShareNetwork,
  Users,
} from "@phosphor-icons/react";
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
  listSessionShareAccess,
  publishSessionShareSnapshot,
  ShareManagementError,
} from "./client";
import { flushCanonicalSessionEditorChanges } from "./editor-activity";
import {
  loadSharePanel,
  requireManagementContext,
  sessionShareManagementQueryKey,
  ShareOperationAbortedError,
  type SharePanelIdentity,
  type SharePreparationIdentity,
} from "./management";
import { SessionSharePopoverContent } from "./management-panel";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  recordPublishedSessionShareState,
} from "./reconciliation";
import { loadSessionShareSource } from "./source";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { env } from "~/env";
import {
  loadManagedSharedNoteForSession,
  upsertDurableSharedNoteCache,
  useDurableSharedNote,
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
  const [sharePreparationIdentity, setSharePreparationIdentity] =
    useState<SharePreparationIdentity | null>(null);
  const [waitingForBilling, setWaitingForBilling] = useState(false);
  const [upgradePromptIdentity, setUpgradePromptIdentity] =
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
  // Drop abandoned preparation state the moment the account or note stops
  // matching, so returning to the original identity cannot auto-resume a
  // publish the user never re-requested.
  if (
    sharePreparationIdentity &&
    (sharePreparationIdentity.ownerUserId !== accountUserId ||
      sharePreparationIdentity.sessionId !== sessionId)
  ) {
    sharePreparationIdentityRef.current = null;
    cancelSharePreparation();
    setSharePreparationIdentity(null);
    setWaitingForBilling(false);
  }
  if (
    upgradePromptIdentity &&
    (upgradePromptIdentity.ownerUserId !== accountUserId ||
      upgradePromptIdentity.sessionId !== sessionId)
  ) {
    setUpgradePromptIdentity(null);
  }
  const activeSharePanelIdentity =
    sharePanelIdentity?.ownerUserId === accountUserId &&
    sharePanelIdentity.sessionId === sessionId
      ? sharePanelIdentity
      : null;
  const activeSharePreparationIdentity =
    sharePreparationIdentity?.ownerUserId === accountUserId &&
    sharePreparationIdentity.sessionId === sessionId
      ? sharePreparationIdentity
      : null;
  const activeUpgradePromptIdentity =
    upgradePromptIdentity?.ownerUserId === accountUserId &&
    upgradePromptIdentity.sessionId === sessionId
      ? upgradePromptIdentity
      : null;
  const showUpgradePrompt =
    Boolean(activeUpgradePromptIdentity) && billing.isReady && !billing.isPaid;
  const sharePopoverOpen =
    showUpgradePrompt ||
    Boolean(activeSharePanelIdentity) ||
    Boolean(activeSharePreparationIdentity);
  const durableNoteQuery = useDurableSharedNote(
    accountUserId,
    activeSharePanelIdentity?.shareId ?? "",
  );
  const initializeMutation = useMutation({
    mutationFn: ({
      publish,
      identity,
    }: {
      publish: boolean;
      identity: SharePreparationIdentity;
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
        const cachedManagedShare = publish
          ? await loadManagedSharedNoteForSession(
              identity.ownerUserId,
              source.sessionId,
            )
          : null;
        context = requireActivePrepareContext(identity, signal);
        if (
          cachedManagedShare &&
          (cachedManagedShare.shareId !== share.shareId ||
            cachedManagedShare.workspaceId !== source.workspaceId ||
            cachedManagedShare.sessionId !== source.sessionId)
        ) {
          throw new ShareManagementError();
        }
        if (publish && (share.wasCreated || !cachedManagedShare)) {
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
            }),
            title: source.title,
            body: source.body,
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
        const access = await listSessionShareAccess(context, share.shareId);
        requireActivePrepareContext(identity, signal);
        return {
          identity: { ...identity, shareId: share.shareId },
          data: { management, access, wasCreated: share.wasCreated },
        };
      }),
    onSuccess: ({ identity, data }) => {
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
      sharePreparationIdentityRef.current = null;
      setSharePreparationIdentity(null);
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
      console.error("[session-sharing] could not prepare note", error);
      sonnerToast.error("Could not prepare this note for sharing.");
    },
  });
  const freeShareMutation = useMutation({
    mutationFn: (identity: SharePreparationIdentity) =>
      loadManagedSharedNoteForSession(identity.ownerUserId, identity.sessionId),
    onSuccess: (managedShare, identity) => {
      if (
        !isActiveSharePreparation(identity) ||
        latestAuthRef.current.session?.user.id !== identity.ownerUserId ||
        latestSessionIdRef.current !== identity.sessionId
      ) {
        clearAbandonedSharePreparation(identity);
        return;
      }
      if (!managedShare) {
        sharePreparationIdentityRef.current = null;
        setSharePreparationIdentity(null);
        setUpgradePromptIdentity(identity);
        return;
      }
      initializeMutation.mutate({ publish: false, identity });
    },
    onError: (_error, identity) => {
      if (
        !isActiveSharePreparation(identity) ||
        latestAuthRef.current.session?.user.id !== identity.ownerUserId ||
        latestSessionIdRef.current !== identity.sessionId
      ) {
        clearAbandonedSharePreparation(identity);
        return;
      }
      sonnerToast.error("Could not check this note's sharing status.");
    },
  });
  const shareButtonPending =
    initializeMutation.isPending || freeShareMutation.isPending;

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
    cancelSharePreparation();
    setSharePanelIdentity(null);
    setSharePreparationIdentity(null);
    setWaitingForBilling(false);
    setUpgradePromptIdentity(null);
    initializeMutation.reset();
    freeShareMutation.reset();
  };

  const runSharePreparation = (identity: SharePreparationIdentity) => {
    if (!isActiveSharePreparation(identity)) return;
    setWaitingForBilling(false);
    if (!billing.isPaid) {
      freeShareMutation.mutate(identity);
      return;
    }
    initializeMutation.mutate({ publish: true, identity });
  };

  const startSharePreparation = (
    identity: Omit<SharePreparationIdentity, "attemptId">,
  ) => {
    cancelSharePreparation();
    initializeMutation.reset();
    freeShareMutation.reset();
    const preparation = {
      ...identity,
      attemptId: nextSharePreparationAttemptRef.current,
    };
    nextSharePreparationAttemptRef.current += 1;
    sharePreparationIdentityRef.current = preparation;
    setSharePreparationIdentity(preparation);
    if (!billing.isReady) {
      setWaitingForBilling(true);
      return;
    }
    runSharePreparation(preparation);
  };

  const handleShare = () => {
    if (sharePopoverOpen) {
      closeSharePopover();
      return;
    }
    if (!auth.session || auth.session.user.is_anonymous === true) {
      void auth.signIn().catch(() => {
        sonnerToast.error("Could not start sign-in.");
      });
      return;
    }
    if (shareButtonPending) return;
    startSharePreparation({
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
          aria-label="Share note"
          aria-expanded={sharePopoverOpen}
          title="Share note"
          onClick={handleShare}
          className={cn([
            "text-muted-foreground hover:text-foreground mr-1 rounded-full",
            sharePopoverOpen && "bg-accent text-foreground",
          ])}
        >
          {shareButtonPending ? (
            <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <ShareNetwork className="size-3.5" aria-hidden="true" />
          )}
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
          loading={shareQuery.isPending}
          error={shareQuery.isError}
          canExpand={billing.isPaid}
          sharedAttachments={sharedAttachments}
          sharedSnapshot={durableNoteQuery.data ?? null}
          sharedAttachmentsReady={sharedAttachmentsReady}
          pendingRef={sharePanelPendingRef}
          onRetry={() => void shareQuery.refetch()}
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
        <>
          {waitingForBilling && billing.isReady ? (
            <SharePreparationStarter
              identity={activeSharePreparationIdentity}
              onStart={runSharePreparation}
            />
          ) : null}
          <SessionSharePreparationContent
            loading={shareButtonPending}
            error={initializeMutation.isError || freeShareMutation.isError}
            onRetry={() =>
              startSharePreparation(activeSharePreparationIdentity)
            }
            onClose={closeSharePopover}
          />
        </>
      ) : null}
    </Popover>
  );
}

function SharePreparationStarter({
  identity,
  onStart,
}: {
  identity: SharePreparationIdentity;
  onStart: (identity: SharePreparationIdentity) => void;
}) {
  const startedRef = useRef(false);
  useMountEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    onStart(identity);
  });
  return null;
}

function SessionSharePreparationContent({
  loading,
  error,
  onRetry,
  onClose,
}: {
  loading: boolean;
  error: boolean;
  onRetry: () => void;
  onClose: () => void;
}) {
  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-heading"
      aria-describedby="session-share-description"
      className="h-[240px] max-h-[calc(100vh-64px)] w-[320px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="flex h-full flex-col overflow-hidden">
        <header className="border-border/60 border-b px-5 py-4 text-left">
          <div className="flex items-center gap-3">
            <div className="bg-accent flex size-9 items-center justify-center rounded-full">
              <Users className="size-4" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <h2
                id="session-share-heading"
                className="text-sm leading-5 font-semibold tracking-normal"
              >
                <Trans>Share note</Trans>
              </h2>
              <p
                id="session-share-description"
                className="text-muted-foreground mt-0.5 text-xs leading-4"
              >
                <Trans>Choose who can open this note.</Trans>
              </p>
            </div>
          </div>
        </header>

        <div className="flex min-h-0 flex-1 items-center justify-center px-5 py-4">
          {error && !loading ? (
            <div className="flex flex-col items-center gap-3 text-center">
              <p className="text-muted-foreground text-xs">
                <Trans>Access settings could not be loaded.</Trans>
              </p>
              <Button size="sm" variant="outline" onClick={onRetry}>
                <ArrowsClockwise className="size-3.5" aria-hidden="true" />
                <Trans>Try again</Trans>
              </Button>
            </div>
          ) : (
            <div className="text-muted-foreground flex items-center gap-2 text-xs">
              <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
              <Trans>Loading access…</Trans>
            </div>
          )}
        </div>

        <footer className="border-border/60 flex justify-end border-t px-5 py-3">
          <Button type="button" size="sm" onClick={onClose}>
            <Check className="size-3.5" aria-hidden="true" />
            <Trans>Done</Trans>
          </Button>
        </footer>
      </AppFloatingPanel>
    </PopoverContent>
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
      className="h-[240px] max-h-[calc(100vh-64px)] w-[320px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="flex h-full flex-col items-center overflow-y-auto px-6 py-7 text-center">
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
