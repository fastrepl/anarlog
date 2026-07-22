import { Trans } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { writeText as writeClipboardText } from "@tauri-apps/plugin-clipboard-manager";
import {
  AlertTriangleIcon,
  CheckIcon,
  CopyIcon,
  ExternalLinkIcon,
  Loader2Icon,
  LockKeyholeIcon,
  RefreshCwIcon,
  Share2Icon,
  UsersIcon,
} from "lucide-react";
import { type MutableRefObject, useCallback, useRef, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { SessionAttachmentControls } from "./attachment-controls";
import {
  addSharedAttachmentIds,
  attachmentMetadataMatches,
  isAttachmentShareable,
  loadSessionShareAttachments,
  matchSharedAttachmentsToLocal,
  prepareSessionShareAttachment,
  type SessionShareAttachment,
  useSessionShareAttachments,
} from "./attachments";
import {
  createOrReuseSessionShare,
  createSessionAccessInvitation,
  enableSessionShareLink,
  getSessionShareManagement,
  listSessionShareAccess,
  publishSessionShareSnapshot,
  resendSessionAccessInvitation,
  reviewSessionAccessRequest,
  revokeSessionAccessGrant,
  revokeSessionAccessInvitation,
  rotateSessionShareLink,
  setSessionShareScope,
  sendSessionAccessInvitationEmail,
  type SessionAccessCapability,
  type SessionShareAccessEntry,
  type SessionShareManagement,
  type ShareManagementContext,
  ShareManagementError,
  updateSessionAccessGrant,
} from "./client";
import { flushCanonicalSessionEditorChanges } from "./editor-activity";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  loadSessionShareSyncState,
  recordPublishedSessionShareState,
} from "./reconciliation";
import { loadSessionShareSource, useAvailableShareWorkspaces } from "./source";
import { useSessionShareSyncStatus } from "./sync-state";
import {
  buildAccountSessionShareUrl,
  buildSessionInvitationUrl,
  buildSessionShareLinkUrl,
  type ShareDesktopScheme,
} from "./urls";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useHumans } from "~/contacts/queries";
import { ContactFacehash, getContactBgClass } from "~/contacts/shared";
import { env } from "~/env";
import { setAttachmentCloudSyncEnabled } from "~/session/attachments";
import {
  loadManagedSharedNoteForSession,
  type SharedNoteAttachment,
  type SharedNoteSnapshot,
  upsertDurableSharedNoteCache,
  useDurableSharedNote,
} from "~/shared-notes/cache";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { getScheme } from "~/shared/utils";

type SharePanelData = {
  management: SessionShareManagement;
  access: SessionShareAccessEntry[];
};

type SharePreparationIdentity = {
  ownerUserId: string;
  sessionId: string;
};

type SharePanelIdentity = SharePreparationIdentity & {
  shareId: string;
};

type AccessMutation =
  | {
      type: "grant-capability";
      entry: Extract<SessionShareAccessEntry, { entryType: "grant" }>;
      capability: SessionAccessCapability;
    }
  | {
      type: "grant-revoke";
      entry: Extract<SessionShareAccessEntry, { entryType: "grant" }>;
    }
  | {
      type: "invitation-capability";
      entry: Extract<SessionShareAccessEntry, { entryType: "invitation" }>;
      capability: SessionAccessCapability;
    }
  | {
      type: "invitation-resend";
      entry: Extract<SessionShareAccessEntry, { entryType: "invitation" }>;
    }
  | {
      type: "invitation-revoke";
      entry: Extract<SessionShareAccessEntry, { entryType: "invitation" }>;
    }
  | {
      type: "request-approve";
      entry: Extract<SessionShareAccessEntry, { entryType: "request" }>;
    }
  | {
      type: "request-deny";
      entry: Extract<SessionShareAccessEntry, { entryType: "request" }>;
    };

type AttachmentMutation =
  | {
      type: "cloud";
      attachment: SessionShareAttachment;
      enabled: boolean;
    }
  | {
      type: "share";
      attachment: SessionShareAttachment;
      included: boolean;
    };

const capabilityLabels: Record<SessionAccessCapability, string> = {
  viewer: "Can view",
  commenter: "Can comment",
  editor: "Can edit",
};

const capabilityRanks: Record<SessionAccessCapability, number> = {
  viewer: 1,
  commenter: 2,
  editor: 3,
};

function isInviteEmail(value: string) {
  const normalized = value.trim();
  return (
    normalized.length <= 320 &&
    /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(normalized) &&
    !/[\u0000-\u001f\u007f]/.test(normalized)
  );
}

class SharePreparationAbortedError extends ShareManagementError {
  constructor() {
    super();
    this.name = "SharePreparationAbortedError";
  }
}

export function sessionShareManagementQueryKey(
  ownerUserId: string,
  shareId: string,
) {
  return ["session-share-management", ownerUserId, shareId] as const;
}

export function SessionShareButton({ sessionId }: { sessionId: string }) {
  const auth = useAuth();
  const latestAuthRef = useRef(auth);
  latestAuthRef.current = auth;
  const latestSessionIdRef = useRef(sessionId);
  latestSessionIdRef.current = sessionId;
  const prepareControllersRef = useRef(new Set<AbortController>());
  const shareButtonLifecycleRef = useCallback(
    (node: HTMLButtonElement | null) => {
      if (node) return;
      for (const controller of prepareControllersRef.current) {
        controller.abort();
      }
      prepareControllersRef.current.clear();
    },
    [],
  );
  const runPrepareOperation = async <T,>(
    operation: (signal: AbortSignal) => Promise<T>,
  ): Promise<T> => {
    const controller = new AbortController();
    prepareControllersRef.current.add(controller);
    try {
      return await operation(controller.signal);
    } catch (error) {
      if (controller.signal.aborted) {
        throw new SharePreparationAbortedError();
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
    setSharePreparationIdentity((current) =>
      current &&
      current.ownerUserId === identity.ownerUserId &&
      current.sessionId === identity.sessionId
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
  const workspaces = useAvailableShareWorkspaces(accountUserId);

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
          data: { management, access },
        };
      }),
    onSuccess: ({ identity, data }) => {
      if (
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
      setSharePreparationIdentity(null);
      setSharePanelIdentity(identity);
    },
    onError: (error, variables) => {
      if (
        error instanceof SharePreparationAbortedError ||
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
        latestAuthRef.current.session?.user.id !== identity.ownerUserId ||
        latestSessionIdRef.current !== identity.sessionId
      ) {
        clearAbandonedSharePreparation(identity);
        return;
      }
      if (!managedShare) {
        setSharePreparationIdentity(null);
        setUpgradePromptIdentity(identity);
        return;
      }
      initializeMutation.mutate({ publish: false, identity });
    },
    onError: (_error, identity) => {
      if (
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
    setSharePanelIdentity(null);
    setSharePreparationIdentity(null);
    setWaitingForBilling(false);
    setUpgradePromptIdentity(null);
    initializeMutation.reset();
    freeShareMutation.reset();
  };

  const runSharePreparation = (identity: SharePreparationIdentity) => {
    setWaitingForBilling(false);
    if (!billing.isPaid) {
      freeShareMutation.mutate(identity);
      return;
    }
    initializeMutation.mutate({ publish: true, identity });
  };

  const startSharePreparation = (identity: SharePreparationIdentity) => {
    initializeMutation.reset();
    freeShareMutation.reset();
    setSharePreparationIdentity(identity);
    if (!billing.isReady) {
      setWaitingForBilling(true);
      return;
    }
    runSharePreparation(identity);
  };

  const handleShare = () => {
    if (sharePopoverOpen) {
      if (!shareButtonPending && !sharePanelPendingRef.current) {
        closeSharePopover();
      }
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
        if (!open && !shareButtonPending && !sharePanelPendingRef.current) {
          closeSharePopover();
        }
      }}
    >
      <PopoverTrigger asChild>
        <Button
          key={accountUserId ?? "signed-out"}
          ref={shareButtonLifecycleRef}
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
            <Loader2Icon className="size-3.5 animate-spin" aria-hidden="true" />
          ) : (
            <Share2Icon className="size-3.5" aria-hidden="true" />
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
          workspaces={workspaces}
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
      onEscapeKeyDown={(event) => {
        if (loading) event.preventDefault();
      }}
      onInteractOutside={(event) => {
        if (loading) event.preventDefault();
      }}
    >
      <AppFloatingPanel className="flex h-full flex-col overflow-hidden">
        <header className="border-border/60 border-b px-5 py-4 text-left">
          <div className="flex items-center gap-3">
            <div className="bg-accent flex size-9 items-center justify-center rounded-full">
              <UsersIcon className="size-4" aria-hidden="true" />
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
                <RefreshCwIcon className="size-3.5" aria-hidden="true" />
                <Trans>Try again</Trans>
              </Button>
            </div>
          ) : (
            <div className="text-muted-foreground flex items-center gap-2 text-xs">
              <Loader2Icon className="size-4 animate-spin" aria-hidden="true" />
              <Trans>Loading access…</Trans>
            </div>
          )}
        </div>

        <footer className="border-border/60 flex justify-end border-t px-5 py-3">
          <Button type="button" size="sm" onClick={onClose} disabled={loading}>
            <CheckIcon className="size-3.5" aria-hidden="true" />
            <Trans>Done</Trans>
          </Button>
        </footer>
      </AppFloatingPanel>
    </PopoverContent>
  );
}

function SessionShareUpgradeContent({ onUpgrade }: { onUpgrade: () => void }) {
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
          <UsersIcon className="size-4" aria-hidden="true" />
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

function SessionSharePopoverContent({
  sessionId,
  identity,
  data,
  loading,
  error,
  canExpand,
  workspaces,
  sharedAttachments,
  sharedSnapshot,
  sharedAttachmentsReady,
  pendingRef,
  onRetry,
  onChanged,
}: {
  sessionId: string;
  identity: SharePanelIdentity;
  data: SharePanelData | undefined;
  loading: boolean;
  error: boolean;
  canExpand: boolean;
  workspaces: Array<{ id: string; name: string }>;
  sharedAttachments: SharedNoteAttachment[];
  sharedSnapshot: SharedNoteSnapshot | null;
  sharedAttachmentsReady: boolean;
  pendingRef: MutableRefObject<boolean>;
  onRetry: () => void;
  onChanged: () => Promise<unknown>;
}) {
  const auth = useAuth();
  const humans = useHumans();
  const latestAuthRef = useRef(auth);
  latestAuthRef.current = auth;
  const operationControllersRef = useRef(new Set<AbortController>());
  const operationLifecycleRef = useCallback(
    (node: HTMLDivElement | null) => {
      if (node) return;
      pendingRef.current = false;
      for (const controller of operationControllersRef.current) {
        controller.abort();
      }
      operationControllersRef.current.clear();
    },
    [pendingRef],
  );
  const runOperation = async <T,>(
    operation: (signal: AbortSignal) => Promise<T>,
  ): Promise<T> => {
    const controller = new AbortController();
    operationControllersRef.current.add(controller);
    try {
      return await operation(controller.signal);
    } finally {
      operationControllersRef.current.delete(controller);
    }
  };
  const requireActiveContext = (signal?: AbortSignal) => {
    if (signal?.aborted) throw new ShareManagementError();
    const context = requireManagementContext(latestAuthRef.current);
    if (context.session.user.id !== identity.ownerUserId) {
      throw new ShareManagementError();
    }
    return { ...context, signal };
  };
  const management = data?.management;
  const syncStatus = useSessionShareSyncStatus(
    identity.ownerUserId,
    identity.shareId,
    sessionId,
  );
  const hasConflict = syncStatus === "conflict";
  const canPublish = canExpand && !hasConflict;
  const { data: sessionAttachments = [] } =
    useSessionShareAttachments(sessionId);
  const sharedAttachmentIds = matchSharedAttachmentsToLocal(
    sessionAttachments,
    sharedAttachments,
  );
  const inviteForm = useForm({
    defaultValues: {
      email: "",
      capability: "viewer" as SessionAccessCapability,
    },
    onSubmit: ({ value }) => {
      inviteMutation.mutate({
        email: value.email,
        capability: value.capability,
      });
    },
  });

  const publishLatest = async (
    signal?: AbortSignal,
    requestedAttachments = sharedAttachments,
    localOverrides = new Map<string, string>(),
    resolveConflict = false,
  ) => {
    if (!identity || !management) throw new ShareManagementError();
    if (!sharedAttachmentsReady || !sharedSnapshot) {
      throw new ShareManagementError();
    }
    await flushCanonicalSessionEditorChanges(sessionId);
    requireActiveContext(signal);
    const syncState = await loadSessionShareSyncState(
      identity.ownerUserId,
      identity.shareId,
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

  const attachmentMutation = useMutation({
    mutationFn: (input: AttachmentMutation) =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        const { attachment } = input;
        const currentId = sharedAttachmentIds.get(attachment.id);
        if (input.type === "cloud") {
          if (!input.enabled && currentId) {
            if (!canExpand) throw new ShareManagementError();
            await publishLatest(
              signal,
              sharedAttachments.filter((item) => item.id !== currentId),
            );
          } else if (input.enabled && !canExpand) {
            throw new ShareManagementError();
          }
          requireActiveContext(signal);
          await setAttachmentCloudSyncEnabled(
            sessionId,
            attachment.id,
            input.enabled,
          );
          requireActiveContext(signal);
          return;
        }

        if (!canExpand) throw new ShareManagementError();
        let requested = [...sharedAttachments];
        if (!input.included) {
          requested = requested.filter((item) => item.id !== currentId);
          return publishLatest(signal, requested);
        }

        const context = requireActiveContext(signal);
        const prepared = await prepareSessionShareAttachment({
          apiBaseUrl: env.VITE_API_URL,
          supabaseUrl: env.VITE_SUPABASE_URL ?? "",
          session: context.session,
          shareId: identity.shareId,
          attachment,
          signal,
        });
        requested = [
          ...requested.filter((item) => item.id !== currentId),
          prepared,
        ];
        return publishLatest(
          signal,
          requested,
          new Map([[attachment.id, prepared.id]]),
        );
      }),
    onSuccess: () => {
      sonnerToast.success("Attachment settings updated.");
    },
    onError: () => {
      sonnerToast.error("Could not update attachment sharing.");
    },
    onSettled: onChanged,
  });

  const inviteMutation = useMutation({
    mutationFn: (input: {
      email: string;
      capability: SessionAccessCapability;
    }) =>
      runOperation(async (signal) => {
        if (!canExpand || !management) throw new ShareManagementError();
        const published = await publishLatest(signal);
        const context = requireActiveContext(signal);
        let invitation = await createSessionAccessInvitation(context, {
          shareId: identity.shareId,
          inviteeEmail: input.email,
          capability: input.capability,
        });
        if (!invitation.inviteToken) {
          invitation = {
            ...(await resendSessionAccessInvitation(
              context,
              invitation.invitationId,
            )),
            wasCreated: true,
          };
        }
        if (!invitation.inviteToken) throw new ShareManagementError();
        try {
          await sendSessionAccessInvitationEmail({
            apiBaseUrl: env.VITE_API_URL,
            session: context.session,
            shareId: identity.shareId,
            invitationId: invitation.invitationId,
            inviteToken: invitation.inviteToken,
            noteTitle: published.title,
            signal,
          });
        } catch {
          await copyInvitationOrRevoke(
            withoutSignal(context),
            {
              invitationId: invitation.invitationId,
              inviteToken: invitation.inviteToken,
            },
            () => requireActiveContext(signal),
          );
          return { deliveredBy: "clipboard" as const };
        }
        requireActiveContext(signal);
        return { deliveredBy: "email" as const };
      }),
    onSuccess: ({ deliveredBy }) => {
      inviteForm.reset();
      sonnerToast.success(
        deliveredBy === "email"
          ? "Invitation sent."
          : "Email unavailable. Invite link copied instead.",
      );
    },
    onError: () => {
      sonnerToast.error("Could not create this invitation.");
    },
    onSettled: onChanged,
  });

  const refreshMutation = useMutation({
    mutationFn: () =>
      runOperation((signal) => {
        if (!canExpand) throw new ShareManagementError();
        return publishLatest(signal);
      }),
    onSuccess: () => {
      sonnerToast.success("Shared copy updated.");
    },
    onError: () => {
      sonnerToast.error("Could not update the shared copy.");
    },
    onSettled: onChanged,
  });

  const keepDesktopMutation = useMutation({
    mutationFn: () =>
      runOperation((signal) =>
        publishLatest(
          signal,
          sharedAttachments,
          new Map<string, string>(),
          true,
        ),
      ),
    onSuccess: () => {
      sonnerToast.success("Desktop edits published. Sharing resumed.");
    },
    onError: () => {
      sonnerToast.error(
        "Could not publish the desktop edits. Check the latest web copy and try again.",
      );
    },
    onSettled: onChanged,
  });

  const openWebCopyMutation = useMutation({
    mutationFn: () =>
      runOperation(async (signal) => {
        requireActiveContext(signal);
        await openerCommands.openUrl(
          buildAccountSessionShareUrl({
            appBaseUrl: env.VITE_APP_URL,
            shareId: identity.shareId,
          }),
          null,
        );
        requireActiveContext(signal);
      }),
    onError: () => {
      sonnerToast.error("Could not open the web copy.");
    },
  });

  // Optimistic General-access value: shown from click until the refreshed
  // management state confirms it, so the select never flashes the old scope.
  const [optimisticScope, setOptimisticScope] = useState<string | null>(null);
  const scopeMutation = useMutation({
    mutationFn: (target: string) =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        let context = requireActiveContext(signal);
        if (target === "restricted") {
          await setSessionShareScope(context, {
            shareId: identity.shareId,
            scope: "restricted",
          });
          return { copied: false };
        }
        if (!canExpand) throw new ShareManagementError();
        await publishLatest(signal);
        context = requireActiveContext(signal);
        if (target === "link") {
          try {
            let link = management.hasActiveLink
              ? await rotateSessionShareLink(context, identity.shareId)
              : await enableSessionShareLink(context, identity.shareId);
            if (!link.linkToken) {
              link = await rotateSessionShareLink(context, identity.shareId);
            }
            const linkToken = link.linkToken;
            if (!linkToken) throw new ShareManagementError();
            requireActiveContext(signal);
            await copyText(
              buildSessionShareLinkUrl({
                appBaseUrl: env.VITE_APP_URL,
                shareId: identity.shareId,
                linkToken,
                desktopScheme: await getSessionShareDesktopScheme(),
              }),
            );
            requireActiveContext(signal);
          } catch {
            await restrictShare(withoutSignal(context), identity.shareId);
            throw new ShareManagementError();
          }
          return { copied: true };
        }

        const scopeInput =
          target === "public"
            ? ({
                shareId: identity.shareId,
                scope: "public" as const,
              } as const)
            : (() => {
                const workspaceId = target.startsWith("workspace:")
                  ? target.slice("workspace:".length)
                  : "";
                if (
                  !workspaces.some((workspace) => workspace.id === workspaceId)
                ) {
                  throw new ShareManagementError();
                }
                return {
                  shareId: identity.shareId,
                  scope: "workspace" as const,
                  workspaceId,
                };
              })();
        try {
          await setSessionShareScope(context, scopeInput);
          requireActiveContext(signal);
        } catch {
          await restrictShare(withoutSignal(context), identity.shareId);
          throw new ShareManagementError();
        }
        return { copied: false };
      }),
    onSuccess: ({ copied }) => {
      sonnerToast.success(copied ? "Share link copied." : "Access updated.");
    },
    onError: () => {
      setOptimisticScope(null);
      sonnerToast.error("Could not update general access.");
    },
    onSettled: async () => {
      await onChanged();
      setOptimisticScope(null);
    },
  });

  const entryMutation = useMutation({
    mutationFn: (input: AccessMutation) =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        let context = requireActiveContext(signal);
        if (input.type === "grant-revoke") {
          await revokeSessionAccessGrant(context, input.entry.entryId);
          return { deliveredBy: "none" as const };
        }
        if (input.type === "grant-capability") {
          const expanding =
            capabilityRanks[input.capability] >
            capabilityRanks[input.entry.capability];
          if (expanding) {
            if (!canExpand) throw new ShareManagementError();
            await publishLatest(signal);
            context = requireActiveContext(signal);
            try {
              await updateSessionAccessGrant(context, {
                grantId: input.entry.entryId,
                capability: input.capability,
              });
              requireActiveContext(signal);
            } catch {
              await updateSessionAccessGrant(withoutSignal(context), {
                grantId: input.entry.entryId,
                capability: input.entry.capability,
              }).catch(() => undefined);
              throw new ShareManagementError();
            }
          } else {
            await updateSessionAccessGrant(context, {
              grantId: input.entry.entryId,
              capability: input.capability,
            });
          }
          return { deliveredBy: "none" as const };
        }
        if (input.type === "invitation-revoke") {
          await revokeSessionAccessInvitation(context, input.entry.entryId);
          return { deliveredBy: "none" as const };
        }
        if (input.type === "invitation-resend") {
          if (!canExpand) throw new ShareManagementError();
          const published = await publishLatest(signal);
          context = requireActiveContext(signal);
          const invitation = await resendSessionAccessInvitation(
            context,
            input.entry.entryId,
          );
          try {
            await sendSessionAccessInvitationEmail({
              apiBaseUrl: env.VITE_API_URL,
              session: context.session,
              shareId: identity.shareId,
              invitationId: invitation.invitationId,
              inviteToken: invitation.inviteToken,
              noteTitle: published.title,
              signal,
            });
          } catch {
            await copyInvitationOrRevoke(
              withoutSignal(context),
              invitation,
              () => requireActiveContext(signal),
            );
            return { deliveredBy: "clipboard" as const };
          }
          requireActiveContext(signal);
          return { deliveredBy: "email" as const };
        }
        if (input.type === "invitation-capability") {
          if (!canExpand) throw new ShareManagementError();
          const published = await publishLatest(signal);
          context = requireActiveContext(signal);
          let invitation = await createSessionAccessInvitation(context, {
            shareId: identity.shareId,
            inviteeEmail: input.entry.userEmail,
            capability: input.capability,
          });
          if (!invitation.inviteToken) {
            invitation = {
              ...(await resendSessionAccessInvitation(
                context,
                invitation.invitationId,
              )),
              wasCreated: true,
            };
          }
          if (!invitation.inviteToken) throw new ShareManagementError();
          try {
            await sendSessionAccessInvitationEmail({
              apiBaseUrl: env.VITE_API_URL,
              session: context.session,
              shareId: identity.shareId,
              invitationId: invitation.invitationId,
              inviteToken: invitation.inviteToken,
              noteTitle: published.title,
              signal,
            });
          } catch {
            await copyInvitationOrRevoke(
              withoutSignal(context),
              {
                invitationId: invitation.invitationId,
                inviteToken: invitation.inviteToken,
              },
              () => requireActiveContext(signal),
            );
            return { deliveredBy: "clipboard" as const };
          }
          requireActiveContext(signal);
          return { deliveredBy: "email" as const };
        }
        if (input.type === "request-deny") {
          await reviewSessionAccessRequest(context, {
            requestId: input.entry.entryId,
            decision: "deny",
          });
          return { deliveredBy: "none" as const };
        }
        if (!canExpand) throw new ShareManagementError();
        await publishLatest(signal);
        context = requireActiveContext(signal);
        const previousGrant = data?.access.find(
          (
            entry,
          ): entry is Extract<
            SessionShareAccessEntry,
            { entryType: "grant" }
          > =>
            entry.entryType === "grant" && entry.userId === input.entry.userId,
        );
        try {
          await reviewSessionAccessRequest(context, {
            requestId: input.entry.entryId,
            decision: "approve",
            capability: input.entry.capability,
          });
          requireActiveContext(signal);
        } catch {
          const rollbackContext = withoutSignal(context);
          if (previousGrant) {
            await updateSessionAccessGrant(rollbackContext, {
              grantId: previousGrant.entryId,
              capability: previousGrant.capability,
            }).catch(() => undefined);
          } else if (input.entry.userId) {
            const currentAccess = await listSessionShareAccess(
              rollbackContext,
              identity.shareId,
            ).catch(() => []);
            const createdGrant = currentAccess.find(
              (entry) =>
                entry.entryType === "grant" &&
                entry.userId === input.entry.userId,
            );
            if (createdGrant?.entryType === "grant") {
              await revokeSessionAccessGrant(
                rollbackContext,
                createdGrant.entryId,
              ).catch(() => undefined);
            }
          }
          throw new ShareManagementError();
        }
        return { deliveredBy: "none" as const };
      }),
    onSuccess: ({ deliveredBy }) => {
      sonnerToast.success(
        deliveredBy === "email"
          ? "Invitation sent."
          : deliveredBy === "clipboard"
            ? "Email unavailable. Invite link copied instead."
            : "Access updated.",
      );
    },
    onError: () => {
      sonnerToast.error("Could not update this person's access.");
    },
    onSettled: onChanged,
  });

  const generalCopyMutation = useMutation({
    mutationFn: () =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        requireActiveContext(signal);
        const desktopScheme = await getSessionShareDesktopScheme();
        requireActiveContext(signal);
        const url = buildAccountSessionShareUrl({
          appBaseUrl: env.VITE_APP_URL,
          shareId: identity.shareId,
          desktopScheme,
        });
        requireActiveContext(signal);
        await copyText(url);
        requireActiveContext(signal);
      }),
    onSuccess: () => {
      sonnerToast.success("Share link copied.");
    },
    onError: () => {
      sonnerToast.error("Could not copy the share link.");
    },
  });

  const anyPending =
    inviteMutation.isPending ||
    refreshMutation.isPending ||
    scopeMutation.isPending ||
    entryMutation.isPending ||
    generalCopyMutation.isPending ||
    attachmentMutation.isPending ||
    keepDesktopMutation.isPending ||
    openWebCopyMutation.isPending;
  pendingRef.current = anyPending;
  const generalScopeValue = management
    ? management.generalScope === "workspace"
      ? `workspace:${management.generalWorkspaceId}`
      : management.generalScope
    : "restricted";
  // The action buttons must track the same scope the select displays, so an
  // optimistic scope switches them together instead of leaving a stale button.
  const shownScopeValue = optimisticScope ?? generalScopeValue;
  const ownerEmail = auth.session?.user.email ?? "";
  const ownerMetadata = auth.session?.user.user_metadata;
  const ownerName =
    typeof ownerMetadata?.full_name === "string" && ownerMetadata.full_name
      ? ownerMetadata.full_name
      : typeof ownerMetadata?.name === "string" && ownerMetadata.name
        ? ownerMetadata.name
        : ownerEmail || "You";
  const existingEmails = new Set(
    data?.access
      .map((entry) => entry.userEmail?.toLowerCase())
      .filter((email): email is string => Boolean(email)) ?? [],
  );
  const suggestedContacts = (query: string) => {
    const normalized = query.trim().toLowerCase();
    if (!normalized || isInviteEmail(normalized)) return [];
    return humans
      .filter(
        (human) =>
          human.email &&
          !existingEmails.has(human.email.toLowerCase()) &&
          human.email.toLowerCase() !== ownerEmail.toLowerCase() &&
          `${human.name}\n${human.email}`.toLowerCase().includes(normalized),
      )
      .slice(0, 4);
  };

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-heading"
      aria-describedby="session-share-description"
      className="h-[240px] max-h-[calc(100vh-64px)] w-[320px] max-w-[calc(100vw-16px)] overflow-hidden"
      onEscapeKeyDown={(event) => {
        if (anyPending) event.preventDefault();
      }}
      onInteractOutside={(event) => {
        if (anyPending) event.preventDefault();
      }}
    >
      <AppFloatingPanel className="flex h-full flex-col overflow-hidden">
        <div ref={operationLifecycleRef} className="contents">
          <header className="border-border/60 border-b px-3 py-2 text-left">
            <h2
              id="session-share-heading"
              className="text-sm leading-5 font-semibold tracking-normal"
            >
              <Trans>Share</Trans>
            </h2>
            <p id="session-share-description" className="sr-only">
              <Trans>Invite people to this note.</Trans>
            </p>
          </header>

          <div className="scrollbar-soft min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-2">
            {loading && !data ? (
              <div className="text-muted-foreground flex min-h-full items-center justify-center gap-2 text-xs">
                <Loader2Icon
                  className="size-4 animate-spin"
                  aria-hidden="true"
                />
                <Trans>Loading access…</Trans>
              </div>
            ) : error || !data || !management ? (
              <div className="flex min-h-full flex-col items-center justify-center gap-3 text-center">
                <p className="text-muted-foreground text-xs">
                  <Trans>Access settings could not be loaded.</Trans>
                </p>
                <Button size="sm" variant="outline" onClick={onRetry}>
                  <RefreshCwIcon className="size-3.5" aria-hidden="true" />
                  <Trans>Try again</Trans>
                </Button>
              </div>
            ) : (
              <div className="space-y-2">
                {hasConflict ? (
                  <section
                    aria-labelledby="sharing-conflict-heading"
                    className="rounded-lg border border-amber-500/35 bg-amber-500/10 px-2.5 py-2"
                  >
                    <div className="flex items-start gap-2.5">
                      <AlertTriangleIcon
                        className="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400"
                        aria-hidden="true"
                      />
                      <div className="min-w-0 flex-1">
                        <h3
                          id="sharing-conflict-heading"
                          className="text-xs font-medium"
                        >
                          <Trans>Sharing paused to protect your edits</Trans>
                        </h3>
                        <p className="text-muted-foreground mt-0.5 text-[11px] leading-4">
                          <Trans>
                            Resolve the web and desktop edits before inviting
                            anyone.
                          </Trans>
                        </p>
                        <div className="mt-2.5 flex flex-wrap gap-2">
                          <Button
                            type="button"
                            size="sm"
                            variant="outline"
                            disabled={openWebCopyMutation.isPending}
                            onClick={() => openWebCopyMutation.mutate()}
                          >
                            {openWebCopyMutation.isPending ? (
                              <Loader2Icon
                                className="size-3.5 animate-spin"
                                aria-hidden="true"
                              />
                            ) : (
                              <ExternalLinkIcon
                                className="size-3.5"
                                aria-hidden="true"
                              />
                            )}
                            <Trans>Open web copy</Trans>
                          </Button>
                          <Button
                            type="button"
                            size="sm"
                            disabled={
                              !canExpand || keepDesktopMutation.isPending
                            }
                            onClick={() => keepDesktopMutation.mutate()}
                          >
                            {keepDesktopMutation.isPending ? (
                              <Loader2Icon
                                className="size-3.5 animate-spin"
                                aria-hidden="true"
                              />
                            ) : null}
                            <Trans>Keep desktop edits</Trans>
                          </Button>
                        </div>
                      </div>
                    </div>
                  </section>
                ) : null}

                <section aria-labelledby="invite-people-heading">
                  <h3 id="invite-people-heading" className="sr-only">
                    <Trans>People with access</Trans>
                  </h3>
                  <form
                    className="flex items-center gap-2"
                    onSubmit={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      void inviteForm.handleSubmit();
                    }}
                  >
                    <inviteForm.Field name="email">
                      {(field) => (
                        <Input
                          type="text"
                          aria-label="Invitee email"
                          autoComplete="email"
                          required
                          value={field.state.value}
                          disabled={!canPublish || inviteMutation.isPending}
                          onBlur={field.handleBlur}
                          onChange={(event) =>
                            field.handleChange(event.target.value)
                          }
                          placeholder="Email or name"
                          className="h-8 min-w-0 flex-1 rounded-md text-xs"
                        />
                      )}
                    </inviteForm.Field>
                    <inviteForm.Subscribe
                      selector={(state) => state.values.email}
                    >
                      {(email) => (
                        <Button
                          type="submit"
                          size="sm"
                          disabled={
                            !canPublish ||
                            !isInviteEmail(email) ||
                            inviteMutation.isPending
                          }
                          className="h-8 shrink-0 rounded-md px-3"
                        >
                          {inviteMutation.isPending ? (
                            <Loader2Icon
                              className="size-3.5 animate-spin"
                              aria-hidden="true"
                            />
                          ) : null}
                          <Trans>Invite</Trans>
                        </Button>
                      )}
                    </inviteForm.Subscribe>
                  </form>

                  <inviteForm.Subscribe
                    selector={(state) => state.values.email}
                  >
                    {(query) => {
                      const suggestions = suggestedContacts(query);
                      return suggestions.length ? (
                        <div className="mt-1 space-y-0.5 rounded-lg border p-1">
                          {suggestions.map((contact) => {
                            const bgClass = getContactBgClass(
                              contact.name || contact.email,
                            );
                            return (
                              <button
                                key={contact.id}
                                type="button"
                                className="hover:bg-accent flex w-full items-center gap-2 rounded-md px-2 py-1 text-left"
                                onClick={() =>
                                  inviteForm.setFieldValue(
                                    "email",
                                    contact.email,
                                  )
                                }
                              >
                                <span
                                  className={cn([
                                    "shrink-0 rounded-full",
                                    bgClass,
                                  ])}
                                >
                                  <ContactFacehash
                                    name={contact.name || contact.email}
                                    size={22}
                                    showInitial={true}
                                    colorClasses={[bgClass]}
                                  />
                                </span>
                                <span className="min-w-0 flex-1">
                                  <span className="block truncate text-xs font-medium">
                                    {contact.name || contact.email}
                                  </span>
                                  {contact.name ? (
                                    <span className="text-muted-foreground block truncate text-[10px]">
                                      {contact.email}
                                    </span>
                                  ) : null}
                                </span>
                              </button>
                            );
                          })}
                        </div>
                      ) : null;
                    }}
                  </inviteForm.Subscribe>

                  <div className="mt-2 space-y-0.5">
                    <div className="flex min-h-9 items-center gap-2 rounded-lg px-1.5 py-1">
                      <span
                        className={cn([
                          "shrink-0 rounded-full",
                          getContactBgClass(ownerName),
                        ])}
                      >
                        <ContactFacehash
                          name={ownerName}
                          size={24}
                          showInitial={true}
                          colorClasses={[getContactBgClass(ownerName)]}
                        />
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-xs font-medium">
                          {ownerName}{" "}
                          <span className="text-muted-foreground">(You)</span>
                        </p>
                        {ownerEmail ? (
                          <p className="text-muted-foreground truncate text-[10px]">
                            {ownerEmail}
                          </p>
                        ) : null}
                      </div>
                      <span className="text-muted-foreground shrink-0 text-[11px]">
                        <Trans>Full access</Trans>
                      </span>
                    </div>

                    {data.access.length
                      ? data.access.map((entry) => (
                          <AccessEntryRow
                            key={`${entry.entryType}:${entry.entryId}`}
                            entry={entry}
                            pending={
                              entryMutation.isPending &&
                              entryMutation.variables?.entry.entryId ===
                                entry.entryId
                            }
                            canExpand={canExpand}
                            contactName={
                              humans.find(
                                (human) =>
                                  human.email.toLowerCase() ===
                                  entry.userEmail?.toLowerCase(),
                              )?.name
                            }
                            onMutate={entryMutation.mutate}
                          />
                        ))
                      : null}
                  </div>
                </section>

                {sessionAttachments.length ? (
                  <SessionAttachmentControls
                    attachments={sessionAttachments}
                    sharedAttachmentIds={sharedAttachmentIds}
                    canUseCloud={canPublish}
                    canInclude={
                      canPublish &&
                      sharedAttachmentsReady &&
                      Boolean(env.VITE_SUPABASE_URL)
                    }
                    cloudPendingAttachmentId={
                      attachmentMutation.isPending &&
                      attachmentMutation.variables?.type === "cloud"
                        ? (attachmentMutation.variables.attachment.id ?? null)
                        : null
                    }
                    sharePendingAttachmentId={
                      attachmentMutation.isPending &&
                      attachmentMutation.variables?.type === "share"
                        ? (attachmentMutation.variables?.attachment.id ?? null)
                        : null
                    }
                    onCloudChange={(attachment, enabled) =>
                      attachmentMutation.mutate({
                        type: "cloud",
                        attachment,
                        enabled,
                      })
                    }
                    onShareChange={(attachment, included) =>
                      attachmentMutation.mutate({
                        type: "share",
                        attachment,
                        included,
                      })
                    }
                  />
                ) : null}

                <section
                  aria-labelledby="general-access-heading"
                  className="border-border/60 border-t pt-2"
                >
                  <h3
                    id="general-access-heading"
                    className="text-muted-foreground mb-1 text-[10px] font-medium"
                  >
                    <Trans>General access</Trans>
                  </h3>
                  <div className="flex items-center gap-2 rounded-lg px-1.5 py-1">
                    <span className="bg-muted flex size-7 shrink-0 items-center justify-center rounded-md">
                      <LockKeyholeIcon
                        className="size-3.5"
                        aria-hidden="true"
                      />
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs font-medium">
                        <Trans>Only people invited</Trans>
                      </p>
                      {shownScopeValue !== "restricted" ? (
                        <p className="text-muted-foreground truncate text-[10px]">
                          <Trans>Previous broad access is still active</Trans>
                        </p>
                      ) : null}
                    </div>
                    {shownScopeValue !== "restricted" ? (
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        disabled={scopeMutation.isPending}
                        onClick={() => {
                          setOptimisticScope("restricted");
                          scopeMutation.mutate("restricted");
                        }}
                        className="h-7 shrink-0 px-2 text-[11px]"
                      >
                        {scopeMutation.isPending ? (
                          <Loader2Icon
                            className="size-3.5 animate-spin"
                            aria-hidden="true"
                          />
                        ) : null}
                        <Trans>Restrict</Trans>
                      </Button>
                    ) : null}
                  </div>
                </section>
              </div>
            )}
          </div>

          <footer className="border-border/60 flex items-center justify-between border-t px-3 py-2">
            <Button
              type="button"
              size="icon"
              variant="ghost"
              disabled={!canPublish || !management || refreshMutation.isPending}
              onClick={() => refreshMutation.mutate()}
              aria-label="Update shared copy"
              title="Update shared copy"
              className="size-7"
            >
              {refreshMutation.isPending ? (
                <Loader2Icon
                  className="size-3.5 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <RefreshCwIcon className="size-3.5" aria-hidden="true" />
              )}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={generalCopyMutation.isPending || !management}
              onClick={() => generalCopyMutation.mutate()}
              className="h-7 rounded-md px-2.5 text-xs"
            >
              {generalCopyMutation.isPending ? (
                <Loader2Icon
                  className="size-3.5 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <CopyIcon className="size-3.5" aria-hidden="true" />
              )}
              <Trans>Copy link</Trans>
            </Button>
          </footer>
        </div>
      </AppFloatingPanel>
    </PopoverContent>
  );
}

function AccessEntryRow({
  entry,
  pending,
  canExpand,
  contactName,
  onMutate,
}: {
  entry: SessionShareAccessEntry;
  pending: boolean;
  canExpand: boolean;
  contactName?: string;
  onMutate: (mutation: AccessMutation) => void;
}) {
  const label = contactName || entry.userEmail || "Anarlog user";
  const bgClass = getContactBgClass(label);
  return (
    <div className="hover:bg-accent/50 flex min-h-9 items-center gap-2 rounded-lg px-1.5 py-1">
      <span className={cn(["shrink-0 rounded-full", bgClass])}>
        <ContactFacehash
          name={label}
          size={24}
          showInitial={true}
          colorClasses={[bgClass]}
        />
      </span>
      <div className="min-w-0 flex-1">
        <p className="truncate text-xs font-medium">{label}</p>
        <p className="text-muted-foreground truncate text-[10px]">
          {contactName && entry.userEmail
            ? entry.userEmail
            : entry.entryType === "grant"
              ? "Anarlog member"
              : entry.entryType === "invitation"
                ? "Invitation pending"
                : `Requested ${capabilityLabels[entry.capability].toLowerCase()}`}
        </p>
      </div>
      {pending ? (
        <Loader2Icon
          className="text-muted-foreground size-3.5 animate-spin"
          aria-label="Updating access"
        />
      ) : entry.entryType === "request" ? (
        <div className="flex items-center gap-1">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={() => onMutate({ type: "request-deny", entry })}
          >
            <Trans>Deny</Trans>
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={!canExpand}
            onClick={() => onMutate({ type: "request-approve", entry })}
          >
            <Trans>Approve</Trans>
          </Button>
        </div>
      ) : (
        <>
          <CapabilitySelect
            value={entry.capability}
            disabled={!canExpand && entry.entryType === "invitation"}
            ariaLabel={`Permission for ${label}`}
            maximumRank={
              canExpand
                ? capabilityRanks.editor
                : capabilityRanks[entry.capability]
            }
            onChange={(capability) =>
              onMutate({
                type:
                  entry.entryType === "grant"
                    ? "grant-capability"
                    : "invitation-capability",
                entry,
                capability,
              } as AccessMutation)
            }
            onResend={
              entry.entryType === "invitation"
                ? () => onMutate({ type: "invitation-resend", entry })
                : undefined
            }
            onRemove={() =>
              onMutate(
                entry.entryType === "grant"
                  ? { type: "grant-revoke", entry }
                  : { type: "invitation-revoke", entry },
              )
            }
          />
        </>
      )}
    </div>
  );
}

function CapabilitySelect({
  value,
  disabled = false,
  ariaLabel,
  maximumRank = capabilityRanks.editor,
  onChange,
  onResend,
  onRemove,
}: {
  value: SessionAccessCapability;
  disabled?: boolean;
  ariaLabel: string;
  maximumRank?: number;
  onChange: (value: SessionAccessCapability) => void;
  onResend?: () => void;
  onRemove?: () => void;
}) {
  return (
    <Select
      value={value}
      disabled={disabled}
      onValueChange={(next) => {
        if (next === "resend") {
          onResend?.();
          return;
        }
        if (next === "remove") {
          onRemove?.();
          return;
        }
        onChange(next as SessionAccessCapability);
      }}
    >
      <SelectTrigger
        aria-label={ariaLabel}
        className="text-muted-foreground h-7 w-auto min-w-[84px] shrink-0 gap-1 rounded-md border-0 bg-transparent px-1.5 text-[11px] shadow-none"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem
          value="viewer"
          disabled={capabilityRanks.viewer > maximumRank}
        >
          Can view
        </SelectItem>
        <SelectItem
          value="commenter"
          disabled={capabilityRanks.commenter > maximumRank}
        >
          Can comment
        </SelectItem>
        <SelectItem
          value="editor"
          disabled={capabilityRanks.editor > maximumRank}
        >
          Can edit
        </SelectItem>
        {onResend || onRemove ? <SelectSeparator /> : null}
        {onResend ? (
          <SelectItem value="resend">Resend invite</SelectItem>
        ) : null}
        {onRemove ? (
          <SelectItem value="remove" className="text-destructive">
            Remove
          </SelectItem>
        ) : null}
      </SelectContent>
    </Select>
  );
}

async function loadSharePanel(
  context: ShareManagementContext,
  shareId: string,
): Promise<SharePanelData> {
  const [management, access] = await Promise.all([
    getSessionShareManagement(context, shareId),
    listSessionShareAccess(context, shareId),
  ]);
  return { management, access };
}

function requireManagementContext(
  auth: ReturnType<typeof useAuth>,
): ShareManagementContext {
  if (
    !auth.supabase ||
    !auth.session ||
    auth.session.user.is_anonymous === true
  ) {
    throw new ShareManagementError();
  }
  return { supabase: auth.supabase, session: auth.session };
}

async function copyText(value: string) {
  if (isTauri()) {
    await writeClipboardText(value);
    return;
  }
  await navigator.clipboard.writeText(value);
}

async function copyInvitationOrRevoke(
  context: ShareManagementContext,
  invitation: { invitationId: string; inviteToken: string },
  assertActive: () => unknown,
) {
  try {
    assertActive();
    await copyText(
      buildSessionInvitationUrl({
        appBaseUrl: env.VITE_APP_URL,
        invitationId: invitation.invitationId,
        inviteToken: invitation.inviteToken,
        desktopScheme: await getSessionShareDesktopScheme(),
      }),
    );
    assertActive();
  } catch {
    await revokeSessionAccessInvitation(context, invitation.invitationId).catch(
      () => undefined,
    );
    throw new ShareManagementError();
  }
}

function withoutSignal(
  context: ShareManagementContext,
): ShareManagementContext {
  return { supabase: context.supabase, session: context.session };
}

async function restrictShare(context: ShareManagementContext, shareId: string) {
  await setSessionShareScope(context, { shareId, scope: "restricted" }).catch(
    () => undefined,
  );
}

async function getSessionShareDesktopScheme(): Promise<ShareDesktopScheme> {
  return (await getScheme()) === "hyprnote-staging"
    ? "hyprnote-staging"
    : "hyprnote";
}
