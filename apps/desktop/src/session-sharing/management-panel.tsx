import { Trans } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  ArrowSquareOut,
  CircleNotch,
  Copy,
  LockKey,
  Warning,
} from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";
import { type MutableRefObject, useCallback, useRef, useState } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import {
  AppFloatingPanel,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

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
  createSessionAccessInvitation,
  listSessionShareAccess,
  publishSessionShareSnapshot,
  resendSessionAccessInvitation,
  reviewSessionAccessRequest,
  revokeSessionAccessGrant,
  revokeSessionAccessInvitation,
  setSessionShareScope,
  sendSessionAccessInvitationEmail,
  type SessionAccessCapability,
  type SessionShareAccessEntry,
  ShareManagementError,
  updateSessionAccessGrant,
} from "./client";
import { flushCanonicalSessionEditorChanges } from "./editor-activity";
import {
  copyInvitationOrRevoke,
  copyText,
  getSessionShareDesktopScheme,
  requireManagementContext,
  ShareOperationAbortedError,
  type SharePanelData,
  type SharePanelIdentity,
  withoutSignal,
} from "./management";
import {
  createSessionShareMutationId,
  hashSessionShareProjection,
  loadSessionShareSyncState,
  recordPublishedSessionShareState,
} from "./reconciliation";
import { loadSessionShareSource } from "./source";
import { useSessionShareSyncStatus } from "./sync-state";
import { buildAccountSessionShareUrl } from "./urls";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useHumans } from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";
import { env } from "~/env";
import { setAttachmentCloudSyncEnabled } from "~/session/attachments";
import {
  type SharedNoteAttachment,
  type SharedNoteSnapshot,
  upsertDurableSharedNoteCache,
} from "~/shared-notes/cache";

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

export function SessionSharePopoverContent({
  sessionId,
  identity,
  data,
  loading,
  error,
  canExpand,
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
      const result = await operation(controller.signal);
      if (controller.signal.aborted) throw new ShareOperationAbortedError();
      return result;
    } catch (error) {
      if (controller.signal.aborted) throw new ShareOperationAbortedError();
      throw error;
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
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
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
            signal,
          );
          return { deliveredBy: "clipboard" as const };
        }
        requireActiveContext(signal);
        return { deliveredBy: "email" as const };
      }),
    onSuccess: ({ deliveredBy }, input) => {
      trackAnalyticsEvent("share_invitation_sent", {
        delivery_method: deliveredBy,
        capability: input.capability,
      });
      inviteForm.reset();
      sonnerToast.success(
        deliveredBy === "email"
          ? "Invitation sent."
          : "Email unavailable. Invite link copied instead.",
      );
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
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
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
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
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
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
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error("Could not open the web copy.");
    },
  });

  // Optimistic General-access value: shown from click until the refreshed
  // management state confirms it, so the select never flashes the old scope.
  const [optimisticScope, setOptimisticScope] = useState<string | null>(null);
  const scopeMutation = useMutation({
    mutationFn: () =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        const context = requireActiveContext(signal);
        await setSessionShareScope(context, {
          shareId: identity.shareId,
          scope: "restricted",
        });
      }),
    onSuccess: () => {
      sonnerToast.success("Access updated.");
    },
    onError: (error) => {
      setOptimisticScope(null);
      if (error instanceof ShareOperationAbortedError) return;
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
              if (signal.aborted) throw new ShareOperationAbortedError();
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
              signal,
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
              signal,
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
          if (signal.aborted) throw new ShareOperationAbortedError();
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
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
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
      trackAnalyticsEvent("share_link_copied", {
        entry_point: "share_panel",
      });
      sonnerToast.success("Share link copied.");
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
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
                <CircleNotch
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
                  <ArrowsClockwise className="size-3.5" aria-hidden="true" />
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
                      <Warning
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
                              <CircleNotch
                                className="size-3.5 animate-spin"
                                aria-hidden="true"
                              />
                            ) : (
                              <ArrowSquareOut
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
                              <CircleNotch
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
                            <CircleNotch
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
                                <ContactFacehash
                                  name={contact.name || contact.email}
                                  size={22}
                                />
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
                      <ContactFacehash name={ownerName} size={24} />
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
                      <LockKey className="size-3.5" aria-hidden="true" />
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
                          scopeMutation.mutate();
                        }}
                        className="h-7 shrink-0 px-2 text-[11px]"
                      >
                        {scopeMutation.isPending ? (
                          <CircleNotch
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
                <CircleNotch
                  className="size-3.5 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <ArrowsClockwise className="size-3.5" aria-hidden="true" />
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
                <CircleNotch
                  className="size-3.5 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <Copy className="size-3.5" aria-hidden="true" />
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
  return (
    <div className="hover:bg-accent/50 flex min-h-9 items-center gap-2 rounded-lg px-1.5 py-1">
      <ContactFacehash name={label} size={24} />
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
        <CircleNotch
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
