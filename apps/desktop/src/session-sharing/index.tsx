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
  Globe2Icon,
  Link2Icon,
  Loader2Icon,
  LockKeyholeIcon,
  RefreshCwIcon,
  Share2Icon,
  Trash2Icon,
  UserPlusIcon,
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
  buildPublicSessionShareUrl,
  buildSessionInvitationUrl,
  buildSessionShareLinkUrl,
  type ShareDesktopScheme,
} from "./urls";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { env } from "~/env";
import { setAttachmentCloudSyncEnabled } from "~/session/attachments";
import {
  loadManagedSharedNoteForSession,
  type SharedNoteAttachment,
  type SharedNoteSnapshot,
  upsertDurableSharedNoteCache,
  useDurableSharedNote,
} from "~/shared-notes/cache";
import { getScheme } from "~/shared/utils";

type SharePanelData = {
  management: SessionShareManagement;
  access: SessionShareAccessEntry[];
};

type SharePanelIdentity = {
  ownerUserId: string;
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
      type: "invitation-copy";
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
    } finally {
      prepareControllersRef.current.delete(controller);
    }
  };
  const requireActivePrepareContext = (
    ownerUserId: string,
    signal: AbortSignal,
  ) => {
    if (signal.aborted) throw new ShareManagementError();
    const context = requireManagementContext(latestAuthRef.current);
    if (context.session.user.id !== ownerUserId) {
      throw new ShareManagementError();
    }
    return { ...context, signal };
  };
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const [sharePanelIdentity, setSharePanelIdentity] =
    useState<SharePanelIdentity | null>(null);
  const [upgradePromptOpen, setUpgradePromptOpen] = useState(false);
  const sharePanelPendingRef = useRef(false);
  const accountUserId = auth.session?.user.id ?? null;
  const activeSharePanelIdentity =
    sharePanelIdentity?.ownerUserId === accountUserId
      ? sharePanelIdentity
      : null;
  const showUpgradePrompt =
    upgradePromptOpen && billing.isReady && !billing.isPaid;
  const sharePopoverOpen =
    showUpgradePrompt || Boolean(activeSharePanelIdentity);
  const durableNoteQuery = useDurableSharedNote(
    accountUserId,
    activeSharePanelIdentity?.shareId ?? "",
  );
  const workspaces = useAvailableShareWorkspaces(accountUserId);

  const initializeMutation = useMutation({
    mutationFn: ({
      publish,
      ownerUserId,
    }: {
      publish: boolean;
      ownerUserId: string;
    }) =>
      runPrepareOperation(async (signal) => {
        let context = requireActivePrepareContext(ownerUserId, signal);
        await flushCanonicalSessionEditorChanges(sessionId);
        context = requireActivePrepareContext(ownerUserId, signal);
        const source = await loadSessionShareSource(sessionId, ownerUserId);
        context = requireActivePrepareContext(ownerUserId, signal);
        const share = await createOrReuseSessionShare(context, {
          workspaceId: source.workspaceId,
          sessionId: source.sessionId,
        });
        context = requireActivePrepareContext(ownerUserId, signal);
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
          ? await loadManagedSharedNoteForSession(ownerUserId, source.sessionId)
          : null;
        context = requireActivePrepareContext(ownerUserId, signal);
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
          context = requireActivePrepareContext(ownerUserId, signal);
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
          context = requireActivePrepareContext(ownerUserId, signal);
        }
        const access = await listSessionShareAccess(context, share.shareId);
        requireActivePrepareContext(ownerUserId, signal);
        return {
          identity: { ownerUserId, shareId: share.shareId },
          data: { management, access },
        };
      }),
    onSuccess: ({ identity, data }) => {
      if (latestAuthRef.current.session?.user.id !== identity.ownerUserId) {
        return;
      }
      queryClient.setQueryData(
        sessionShareManagementQueryKey(identity.ownerUserId, identity.shareId),
        data,
      );
      void queryClient.invalidateQueries({
        queryKey: ["durable-shared-note-cache", identity.ownerUserId],
      });
      setSharePanelIdentity(identity);
    },
    onError: (error, variables) => {
      if (latestAuthRef.current.session?.user.id !== variables.ownerUserId) {
        return;
      }
      console.error("[session-sharing] could not prepare note", error);
      sonnerToast.error("Could not prepare this note for sharing.");
    },
  });
  const freeShareMutation = useMutation({
    mutationFn: (ownerUserId: string) =>
      loadManagedSharedNoteForSession(ownerUserId, sessionId),
    onSuccess: (managedShare, ownerUserId) => {
      if (latestAuthRef.current.session?.user.id !== ownerUserId) return;
      if (!managedShare) {
        setUpgradePromptOpen(true);
        return;
      }
      initializeMutation.mutate({ publish: false, ownerUserId });
    },
    onError: () => {
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

  const handleShare = () => {
    if (sharePopoverOpen) {
      if (!sharePanelPendingRef.current) {
        setSharePanelIdentity(null);
        setUpgradePromptOpen(false);
      }
      return;
    }
    if (!billing.isReady || shareButtonPending) return;
    if (!auth.session || auth.session.user.is_anonymous === true) {
      void auth.signIn().catch(() => {
        sonnerToast.error("Could not start sign-in.");
      });
      return;
    }
    if (!billing.isPaid) {
      freeShareMutation.mutate(auth.session.user.id);
      return;
    }
    initializeMutation.mutate({
      publish: true,
      ownerUserId: auth.session.user.id,
    });
  };

  return (
    <Popover
      open={sharePopoverOpen}
      onOpenChange={(open) => {
        if (!open && !sharePanelPendingRef.current) {
          setSharePanelIdentity(null);
          setUpgradePromptOpen(false);
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
          key={`${activeSharePanelIdentity.ownerUserId}:${activeSharePanelIdentity.shareId}:${sessionId}`}
          sessionId={sessionId}
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
          onClose={() => setSharePanelIdentity(null)}
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
      ) : null}
    </Popover>
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
      className="w-[320px] overflow-hidden"
    >
      <AppFloatingPanel className="flex flex-col items-center px-6 py-7 text-center">
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
  onClose,
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
  onClose: () => void;
  onChanged: () => Promise<unknown>;
}) {
  const auth = useAuth();
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
        await publishLatest(signal);
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
        await copyInvitationOrRevoke(
          withoutSignal(context),
          {
            invitationId: invitation.invitationId,
            inviteToken: invitation.inviteToken,
          },
          () => requireActiveContext(signal),
        );
      }),
    onSuccess: () => {
      inviteForm.reset();
      sonnerToast.success("Invite link copied.");
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
      sonnerToast.error("Could not update general access.");
    },
    onSettled: onChanged,
  });

  const entryMutation = useMutation({
    mutationFn: (input: AccessMutation) =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        let context = requireActiveContext(signal);
        if (input.type === "grant-revoke") {
          await revokeSessionAccessGrant(context, input.entry.entryId);
          return { copied: false };
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
          return { copied: false };
        }
        if (input.type === "invitation-revoke") {
          await revokeSessionAccessInvitation(context, input.entry.entryId);
          return { copied: false };
        }
        if (input.type === "invitation-copy") {
          if (!canExpand) throw new ShareManagementError();
          await publishLatest(signal);
          context = requireActiveContext(signal);
          const invitation = await resendSessionAccessInvitation(
            context,
            input.entry.entryId,
          );
          await copyInvitationOrRevoke(withoutSignal(context), invitation, () =>
            requireActiveContext(signal),
          );
          return { copied: true };
        }
        if (input.type === "invitation-capability") {
          if (!canExpand) throw new ShareManagementError();
          if (
            capabilityRanks[input.capability] >
            capabilityRanks[input.entry.capability]
          ) {
            await publishLatest(signal);
            context = requireActiveContext(signal);
          }
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
          await copyInvitationOrRevoke(
            withoutSignal(context),
            {
              invitationId: invitation.invitationId,
              inviteToken: invitation.inviteToken,
            },
            () => requireActiveContext(signal),
          );
          return { copied: true };
        }
        if (input.type === "request-deny") {
          await reviewSessionAccessRequest(context, {
            requestId: input.entry.entryId,
            decision: "deny",
          });
          return { copied: false };
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
        return { copied: false };
      }),
    onSuccess: ({ copied }) => {
      sonnerToast.success(copied ? "Invite link copied." : "Access updated.");
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
        const url =
          management.generalScope === "public"
            ? buildPublicSessionShareUrl({
                appBaseUrl: env.VITE_APP_URL,
                publicSlug: management.publicSlug,
                desktopScheme,
              })
            : management.generalScope === "workspace"
              ? buildAccountSessionShareUrl({
                  appBaseUrl: env.VITE_APP_URL,
                  shareId: identity.shareId,
                  desktopScheme,
                })
              : null;
        if (!url) throw new ShareManagementError();
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

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-heading"
      aria-describedby="session-share-description"
      className="w-[min(500px,calc(100vw-16px))] overflow-hidden"
      onEscapeKeyDown={(event) => {
        if (anyPending) event.preventDefault();
      }}
      onInteractOutside={(event) => {
        if (anyPending) event.preventDefault();
      }}
    >
      <AppFloatingPanel className="flex max-h-[calc(100vh-64px)] flex-col overflow-hidden">
        <div ref={operationLifecycleRef} className="contents">
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

          <div className="scrollbar-soft min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-4">
            {loading && !data ? (
              <div className="text-muted-foreground flex min-h-52 items-center justify-center gap-2 text-xs">
                <Loader2Icon
                  className="size-4 animate-spin"
                  aria-hidden="true"
                />
                <Trans>Loading access…</Trans>
              </div>
            ) : error || !data || !management ? (
              <div className="flex min-h-52 flex-col items-center justify-center gap-3 text-center">
                <p className="text-muted-foreground text-xs">
                  <Trans>Access settings could not be loaded.</Trans>
                </p>
                <Button size="sm" variant="outline" onClick={onRetry}>
                  <RefreshCwIcon className="size-3.5" aria-hidden="true" />
                  <Trans>Try again</Trans>
                </Button>
              </div>
            ) : (
              <div className="space-y-6">
                {hasConflict ? (
                  <section
                    aria-labelledby="sharing-conflict-heading"
                    className="rounded-xl border border-amber-500/35 bg-amber-500/10 px-3 py-3"
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
                        <p className="text-muted-foreground mt-1 text-xs leading-5">
                          <Trans>
                            This note changed on both desktop and the web. Open
                            the web copy to review it. To keep that version,
                            close this panel, replace this note with the web
                            content, then switch to another note so Anarlog can
                            reconcile. Otherwise, publish the desktop edits over
                            it.
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
                  <div className="mb-3 flex items-center justify-between gap-3">
                    <div>
                      <h3
                        id="invite-people-heading"
                        className="text-sm font-medium"
                      >
                        <Trans>People with access</Trans>
                      </h3>
                      <p className="text-muted-foreground mt-0.5 text-xs">
                        <Trans>
                          Invite links are copied so you can send them
                          privately.
                        </Trans>
                      </p>
                    </div>
                  </div>
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
                          type="email"
                          aria-label="Invitee email"
                          autoComplete="email"
                          required
                          value={field.state.value}
                          disabled={!canPublish || inviteMutation.isPending}
                          onBlur={field.handleBlur}
                          onChange={(event) =>
                            field.handleChange(event.target.value)
                          }
                          placeholder="name@example.com"
                          className="h-8 min-w-0 flex-1 rounded-full text-xs"
                        />
                      )}
                    </inviteForm.Field>
                    <inviteForm.Field name="capability">
                      {(field) => (
                        <CapabilitySelect
                          value={field.state.value}
                          disabled={!canPublish || inviteMutation.isPending}
                          ariaLabel="Invite permission"
                          onChange={field.handleChange}
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
                            !email.trim() ||
                            inviteMutation.isPending
                          }
                          className="shrink-0"
                        >
                          {inviteMutation.isPending ? (
                            <Loader2Icon
                              className="size-3.5 animate-spin"
                              aria-hidden="true"
                            />
                          ) : (
                            <UserPlusIcon
                              className="size-3.5"
                              aria-hidden="true"
                            />
                          )}
                          <Trans>Copy invite</Trans>
                        </Button>
                      )}
                    </inviteForm.Subscribe>
                  </form>

                  {data.access.length ? (
                    <div className="mt-4 space-y-1">
                      {data.access.map((entry) => (
                        <AccessEntryRow
                          key={`${entry.entryType}:${entry.entryId}`}
                          entry={entry}
                          pending={
                            entryMutation.isPending &&
                            entryMutation.variables?.entry.entryId ===
                              entry.entryId
                          }
                          canExpand={canExpand}
                          onMutate={entryMutation.mutate}
                        />
                      ))}
                    </div>
                  ) : (
                    <p className="text-muted-foreground mt-4 rounded-xl border border-dashed px-3 py-4 text-center text-xs">
                      <Trans>No one has individual access yet.</Trans>
                    </p>
                  )}
                </section>

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

                <section
                  aria-labelledby="general-access-heading"
                  className="border-border/60 border-t pt-5"
                >
                  <h3
                    id="general-access-heading"
                    className="text-sm font-medium"
                  >
                    <Trans>General access</Trans>
                  </h3>
                  <div className="mt-3 flex items-center gap-2">
                    <Select
                      value={generalScopeValue}
                      disabled={scopeMutation.isPending}
                      onValueChange={scopeMutation.mutate}
                    >
                      <SelectTrigger
                        aria-label="General access"
                        className="h-8 min-w-0 flex-1 rounded-full text-xs"
                      >
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="restricted">Restricted</SelectItem>
                        <SelectItem value="link" disabled={!canPublish}>
                          Anyone with the link
                        </SelectItem>
                        {workspaces.map((workspace) => (
                          <SelectItem
                            key={workspace.id}
                            value={`workspace:${workspace.id}`}
                            disabled={!canPublish}
                          >
                            {workspace.name}
                          </SelectItem>
                        ))}
                        <SelectItem value="public" disabled={!canPublish}>
                          Public — searchable on the web
                        </SelectItem>
                      </SelectContent>
                    </Select>
                    {management.generalScope === "link" ? (
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        disabled={!canPublish || scopeMutation.isPending}
                        onClick={() => scopeMutation.mutate("link")}
                        className="shrink-0"
                      >
                        <RefreshCwIcon
                          className="size-3.5"
                          aria-hidden="true"
                        />
                        <Trans>Replace link & copy</Trans>
                      </Button>
                    ) : management.generalScope === "public" ||
                      management.generalScope === "workspace" ? (
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        disabled={generalCopyMutation.isPending}
                        onClick={() => generalCopyMutation.mutate()}
                        className="shrink-0"
                      >
                        <CopyIcon className="size-3.5" aria-hidden="true" />
                        <Trans>Copy link</Trans>
                      </Button>
                    ) : null}
                  </div>
                  <GeneralAccessDescription management={management} />
                </section>

                <div className="bg-muted/60 text-muted-foreground rounded-xl px-3 py-2.5 text-xs leading-5">
                  {!canExpand ? (
                    <p className="text-foreground mb-1">
                      <Trans>
                        Upgrade to expand access. You can still restrict or
                        revoke existing access.
                      </Trans>
                    </p>
                  ) : null}
                  <p>
                    <Trans>
                      Attachments stay private unless you explicitly include
                      them in this shared note.
                    </Trans>
                  </p>
                  <p className="mt-1">
                    <Trans>
                      Invited editors can edit on the web. If both copies
                      change, sharing pauses until you resolve it.
                    </Trans>
                  </p>
                </div>
              </div>
            )}
          </div>

          <footer className="border-border/60 flex items-center justify-between border-t px-5 py-3">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={!canPublish || !management || refreshMutation.isPending}
              onClick={() => refreshMutation.mutate()}
            >
              {refreshMutation.isPending ? (
                <Loader2Icon
                  className="size-3.5 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <RefreshCwIcon className="size-3.5" aria-hidden="true" />
              )}
              <Trans>Update shared copy</Trans>
            </Button>
            <Button
              type="button"
              size="sm"
              onClick={onClose}
              disabled={anyPending}
            >
              <CheckIcon className="size-3.5" aria-hidden="true" />
              <Trans>Done</Trans>
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
  onMutate,
}: {
  entry: SessionShareAccessEntry;
  pending: boolean;
  canExpand: boolean;
  onMutate: (mutation: AccessMutation) => void;
}) {
  const label = entry.userEmail ?? "Anarlog user";
  return (
    <div className="hover:bg-accent/50 flex min-h-11 items-center gap-2 rounded-xl px-2 py-1.5">
      <div className="bg-accent flex size-7 shrink-0 items-center justify-center rounded-full">
        {entry.entryType === "invitation" ? (
          <UserPlusIcon className="size-3.5" aria-hidden="true" />
        ) : (
          <UsersIcon className="size-3.5" aria-hidden="true" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-xs font-medium">{label}</p>
        <p className="text-muted-foreground text-[11px]">
          {entry.entryType === "grant"
            ? "Active"
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
          />
          {entry.entryType === "invitation" ? (
            <Button
              type="button"
              size="icon"
              variant="ghost"
              disabled={!canExpand}
              aria-label={`Copy a new invite for ${label}`}
              title="Copy a new invite link"
              onClick={() => onMutate({ type: "invitation-copy", entry })}
            >
              <CopyIcon className="size-3.5" aria-hidden="true" />
            </Button>
          ) : null}
          <Button
            type="button"
            size="icon"
            variant="ghost"
            aria-label={`Remove access for ${label}`}
            title="Remove access"
            onClick={() =>
              onMutate(
                entry.entryType === "grant"
                  ? { type: "grant-revoke", entry }
                  : { type: "invitation-revoke", entry },
              )
            }
            className="text-muted-foreground hover:text-destructive"
          >
            <Trash2Icon className="size-3.5" aria-hidden="true" />
          </Button>
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
}: {
  value: SessionAccessCapability;
  disabled?: boolean;
  ariaLabel: string;
  maximumRank?: number;
  onChange: (value: SessionAccessCapability) => void;
}) {
  return (
    <Select
      value={value}
      disabled={disabled}
      onValueChange={(next) => onChange(next as SessionAccessCapability)}
    >
      <SelectTrigger
        aria-label={ariaLabel}
        className="h-8 w-[112px] shrink-0 rounded-full text-xs"
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
      </SelectContent>
    </Select>
  );
}

function GeneralAccessDescription({
  management,
}: {
  management: SessionShareManagement;
}) {
  const content = (() => {
    if (management.generalScope === "restricted") {
      return {
        icon: LockKeyholeIcon,
        text: "Only invited people can open this note.",
      };
    }
    if (management.generalScope === "link") {
      return {
        icon: Link2Icon,
        text: "Anyone with the link can view. Replacing it invalidates the old link.",
      };
    }
    if (management.generalScope === "workspace") {
      return {
        icon: UsersIcon,
        text: "Members of this workspace can view the note.",
      };
    }
    return {
      icon: Globe2Icon,
      text: "Anyone can view this note, and search engines may index it.",
    };
  })();
  const Icon = content.icon;
  return (
    <p className="text-muted-foreground mt-2 flex items-start gap-1.5 text-xs leading-5">
      <Icon className="mt-1 size-3 shrink-0" aria-hidden="true" />
      {content.text}
    </p>
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
