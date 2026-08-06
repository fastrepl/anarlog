import { Trans } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  ArrowSquareOut,
  CircleNotch,
  Copy,
  Warning,
} from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { type MutableRefObject, useState } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  PopoverContent,
} from "@anlg/ui/components/ui/popover";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  AccessEntryRow,
  useSessionAccessManagement,
} from "./access-management";
import { SessionAttachmentControls } from "./attachment-controls";
import { useSessionAttachmentManagement } from "./attachment-management";
import {
  matchSharedAttachmentsToLocal,
  useSessionShareAttachments,
} from "./attachments";
import { setSessionShareScope, ShareManagementError } from "./client";
import { useSessionRecapDelivery } from "./delivery-management";
import {
  EmailRecapForm,
  ShareRecapModeSelector,
  SlackRecapForm,
  type ShareRecapMode,
} from "./delivery-panel";
import {
  generalAccessWorkspaceId,
  GeneralAccessSelector,
  type GeneralAccessTarget,
  type GeneralAccessValue,
} from "./general-access";
import { useSessionInvitationManagement } from "./invitation-management";
import {
  ShareInviteForm,
  ShareInviteRecipientRows,
  useShareInvite,
} from "./invite-recipients";
import {
  copyPublicSessionShareUrl,
  copySessionShareUrl,
  enableAndCopySessionShareLink,
  ShareOperationAbortedError,
  type SharePanelData,
  type SharePanelIdentity,
  withoutSignal,
} from "./management";
import { useShareOperationLifecycle } from "./management-operation";
import { createPublishLatestSessionShare } from "./management-publish";
import type { AvailableShareWorkspace } from "./source";
import { useSessionShareSyncStatus } from "./sync-state";
import { buildAccountSessionShareUrl } from "./urls";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useHumans } from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";
import { env } from "~/env";
import {
  type SharedNoteAttachment,
  type SharedNoteSnapshot,
} from "~/shared-notes/cache";

export function SessionSharePopoverContent({
  sessionId,
  identity,
  data,
  error,
  canExpand,
  sharedAttachments,
  sharedSnapshot,
  sharedAttachmentsReady,
  workspaces,
  pendingRef,
  onRetry,
  onActivated,
  onChanged,
}: {
  sessionId: string;
  identity: SharePanelIdentity;
  data: SharePanelData | undefined;
  error: boolean;
  canExpand: boolean;
  sharedAttachments: SharedNoteAttachment[];
  sharedSnapshot: SharedNoteSnapshot | null;
  sharedAttachmentsReady: boolean;
  workspaces: AvailableShareWorkspace[];
  pendingRef: MutableRefObject<boolean>;
  onRetry: () => void;
  onActivated: () => Promise<unknown>;
  onChanged: () => Promise<unknown>;
}) {
  const auth = useAuth();
  const humans = useHumans();
  const { operationLifecycleRef, runOperation, requireActiveContext } =
    useShareOperationLifecycle({ auth, identity, pendingRef });
  const management = data?.management;
  const syncStatus = useSessionShareSyncStatus(
    identity.ownerUserId,
    identity.shareId,
    sessionId,
  );
  const hasConflict = syncStatus === "conflict";
  const canPublish = canExpand && !hasConflict && Boolean(management);
  const [recapMode, setRecapMode] = useState<ShareRecapMode>("invite");
  const { data: sessionAttachments = [] } =
    useSessionShareAttachments(sessionId);
  const sharedAttachmentIds = matchSharedAttachmentsToLocal(
    sessionAttachments,
    sharedAttachments,
  );
  const publishLatest = createPublishLatestSessionShare({
    sessionId,
    identity,
    management,
    sharedAttachmentsReady,
    sharedSnapshot,
    sharedAttachments,
    requireActiveContext,
  });

  const attachmentMutation = useSessionAttachmentManagement({
    identity,
    managementAvailable: Boolean(management),
    canExpand,
    sharedAttachments,
    sharedAttachmentIds,
    runOperation,
    publishLatest,
    requireActiveContext,
    onChanged,
  });
  const { inviteMutation } = useSessionInvitationManagement({
    identity,
    managementAvailable: Boolean(management),
    canExpand,
    runOperation,
    publishLatest,
    requireActiveContext,
    onActivated,
    onChanged,
  });
  const { emailMutation, slackMutation } = useSessionRecapDelivery({
    shareId: identity.shareId,
    canDeliver: canPublish,
    runOperation,
    publishLatest,
    requireActiveContext,
    onActivated,
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
  const [optimisticScope, setOptimisticScope] =
    useState<GeneralAccessValue | null>(null);
  const scopeMutation = useMutation({
    mutationFn: (target: GeneralAccessTarget) =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        let context = requireActiveContext(signal);
        if (target === "restricted") {
          await setSessionShareScope(context, {
            shareId: identity.shareId,
            scope: "restricted",
          });
          await onActivated();
          return { copied: false };
        }
        if (!canExpand) throw new ShareManagementError();
        await publishLatest(signal);
        context = requireActiveContext(signal);
        if (target === "link") {
          await enableAndCopySessionShareLink({
            context,
            shareId: identity.shareId,
            hasActiveLink: management.hasActiveLink,
            assertActive: () => requireActiveContext(signal),
          });
          await onActivated();
          return { copied: true };
        }
        const workspaceId = generalAccessWorkspaceId(target, workspaces);
        if (!workspaceId) throw new ShareManagementError();
        try {
          await setSessionShareScope(context, {
            shareId: identity.shareId,
            scope: "workspace",
            workspaceId,
          });
          requireActiveContext(signal);
        } catch {
          await setSessionShareScope(withoutSignal(context), {
            shareId: identity.shareId,
            scope: "restricted",
          }).catch(() => undefined);
          throw new ShareManagementError();
        }
        await onActivated();
        return { copied: false };
      }),
    onSuccess: ({ copied }) => {
      sonnerToast.success(
        copied
          ? "Anyone with the link can view. Link copied."
          : "Access updated.",
      );
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

  const entryMutation = useSessionAccessManagement({
    identity,
    data,
    management,
    canExpand,
    runOperation,
    publishLatest,
    requireActiveContext,
    onChanged,
  });

  const generalCopyMutation = useMutation({
    mutationFn: () =>
      runOperation(async (signal) => {
        if (!management) throw new ShareManagementError();
        if (management.generalScope === "public") {
          await copyPublicSessionShareUrl(management.publicSlug, () =>
            requireActiveContext(signal),
          );
        } else {
          await copySessionShareUrl(identity.shareId, () =>
            requireActiveContext(signal),
          );
        }
        await onActivated();
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
    scopeMutation.isPending ||
    entryMutation.isPending ||
    generalCopyMutation.isPending ||
    attachmentMutation.isPending ||
    keepDesktopMutation.isPending ||
    openWebCopyMutation.isPending;
  const deliveryPending = emailMutation.isPending || slackMutation.isPending;
  pendingRef.current = anyPending || deliveryPending;
  const generalScopeValue: GeneralAccessValue = management
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
  const invite = useShareInvite({
    sessionId,
    ownerEmail,
    invitedEmails:
      data?.access
        .map((entry) => entry.userEmail ?? "")
        .filter((email) => Boolean(email)) ?? [],
  });

  return (
    <PopoverContent
      variant="app"
      align="end"
      sideOffset={8}
      aria-labelledby="session-share-heading"
      aria-describedby="session-share-description"
      className="w-[440px] max-w-[calc(100vw-16px)] overflow-hidden"
    >
      <AppFloatingPanel className="flex max-h-[min(530px,calc(100vh-74px))] flex-col overflow-hidden">
        <div ref={operationLifecycleRef} className="contents">
          <h2 id="session-share-heading" className="sr-only">
            <Trans>Share</Trans>
          </h2>
          <p id="session-share-description" className="sr-only">
            <Trans>Invite people to this note.</Trans>
          </p>

          <div className="scrollbar-soft min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-2">
            <div className="space-y-2">
              {error && !data ? (
                <div className="border-destructive/30 bg-destructive/5 flex items-center justify-between gap-2 rounded-lg border px-2.5 py-2">
                  <p className="text-muted-foreground text-xs">
                    <Trans>Access settings could not be loaded.</Trans>
                  </p>
                  <Button size="sm" variant="outline" onClick={onRetry}>
                    <ArrowsClockwise className="size-3.5" aria-hidden="true" />
                    <Trans>Try again</Trans>
                  </Button>
                </div>
              ) : null}
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
                          disabled={!canExpand || keepDesktopMutation.isPending}
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

              <ShareRecapModeSelector
                value={recapMode}
                onValueChange={setRecapMode}
              />

              {recapMode === "invite" ? (
                <section aria-labelledby="invite-people-heading">
                  <h3 id="invite-people-heading" className="sr-only">
                    <Trans>People with access</Trans>
                  </h3>
                  <ShareInviteForm
                    invite={invite}
                    disabled={!canPublish || inviteMutation.isPending}
                    pending={inviteMutation.isPending}
                    onSubmit={(emails) => {
                      inviteMutation.mutate(
                        { emails, capability: "viewer" },
                        {
                          onSuccess: (deliveries) => {
                            for (const delivery of deliveries) {
                              if (delivery.deliveredBy) {
                                invite.remove(delivery.email);
                              }
                            }
                          },
                        },
                      );
                    }}
                  />

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

                    <ShareInviteRecipientRows
                      invite={invite}
                      disabled={!canPublish || inviteMutation.isPending}
                    />

                    {data?.access.length
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
                            onMutate={(mutation) => {
                              entryMutation.mutate(mutation, {
                                onSuccess: () => {
                                  if (
                                    (mutation.type === "grant-revoke" ||
                                      mutation.type === "invitation-revoke") &&
                                    mutation.entry.userEmail
                                  ) {
                                    invite.restore(mutation.entry.userEmail);
                                  }
                                },
                              });
                            }}
                          />
                        ))
                      : null}
                  </div>
                </section>
              ) : recapMode === "email" ? (
                <EmailRecapForm
                  sessionId={sessionId}
                  ownerEmail={ownerEmail}
                  disabled={!canPublish || anyPending || deliveryPending}
                  pending={emailMutation.isPending}
                  onSubmit={(emails) => emailMutation.mutate(emails)}
                />
              ) : (
                <SlackRecapForm
                  disabled={!canPublish || anyPending || deliveryPending}
                  pending={slackMutation.isPending}
                  onSubmit={(channel) => slackMutation.mutate(channel)}
                />
              )}

              {sessionAttachments.length ? (
                <SessionAttachmentControls
                  attachments={sessionAttachments}
                  sharedAttachmentIds={sharedAttachmentIds}
                  canShare={
                    canPublish &&
                    sharedAttachmentsReady &&
                    Boolean(env.VITE_SUPABASE_URL)
                  }
                  pendingAttachmentId={
                    attachmentMutation.isPending
                      ? (attachmentMutation.variables?.attachment.id ?? null)
                      : null
                  }
                  onShareChange={(attachment, included) =>
                    attachmentMutation.mutate({
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
                <GeneralAccessSelector
                  value={shownScopeValue}
                  workspaces={workspaces}
                  disabled={!management}
                  canExpand={canPublish}
                  pending={scopeMutation.isPending}
                  onValueChange={(target) => {
                    setOptimisticScope(target);
                    scopeMutation.mutate(target);
                  }}
                />
              </section>
            </div>
          </div>

          <footer className="border-border/60 flex items-center justify-end border-t px-3 py-2">
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={
                generalCopyMutation.isPending ||
                scopeMutation.isPending ||
                !management
              }
              onClick={() => {
                if (shownScopeValue === "link") {
                  scopeMutation.mutate("link");
                } else {
                  generalCopyMutation.mutate();
                }
              }}
              className="h-7 rounded-md px-2.5 text-xs"
            >
              {generalCopyMutation.isPending || scopeMutation.isPending ? (
                <CircleNotch
                  className="size-3.5 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <Copy className="size-3.5" aria-hidden="true" />
              )}
              {shownScopeValue === "link" ? (
                <Trans>Replace link &amp; copy</Trans>
              ) : (
                <Trans>Copy link</Trans>
              )}
            </Button>
          </footer>
        </div>
      </AppFloatingPanel>
    </PopoverContent>
  );
}
