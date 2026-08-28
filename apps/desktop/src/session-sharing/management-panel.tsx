import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  ArrowSquareOut,
  CircleNotch,
  Copy,
  Warning,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { type MutableRefObject, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
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
import {
  getSessionShareWorkspaceSlug,
  setSessionShareScope,
  ShareManagementError,
} from "./client";
import { useSessionRecapDelivery } from "./delivery-management";
import {
  EmailRecapForm,
  ShareRecapOverflowMenu,
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
  ShareInviteSuggestions,
  useShareInvite,
} from "./invite-recipients";
import {
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
import { useWorkspaceShareScopes } from "./workspace-policy";

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
  const allowedScopes = useWorkspaceShareScopes(workspaces);
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
      sonnerToast.success(t`Desktop edits published. Sharing resumed.`);
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(
        t`Could not publish the desktop edits. Check the latest web copy and try again.`,
        { id: "desktop-edits-publish-failed" },
      );
    },
    onSettled: onChanged,
  });

  const openWebCopyMutation = useMutation({
    mutationFn: () =>
      runOperation(async (signal) => {
        const context = requireActiveContext(signal);
        const workspaceShareSlug = await getSessionShareWorkspaceSlug(
          context,
          identity.shareId,
        );
        requireActiveContext(signal);
        await openerCommands.openUrl(
          buildAccountSessionShareUrl({
            appBaseUrl: env.VITE_APP_URL,
            shareId: identity.shareId,
            workspaceShareSlug,
          }),
          null,
        );
        requireActiveContext(signal);
      }),
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(t`Could not open the web copy.`);
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
          ? t`Anyone with the link can view. Link copied.`
          : t`Access updated.`,
      );
    },
    onError: (error) => {
      setOptimisticScope(null);
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(t`Could not update general access.`);
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
        const context = requireActiveContext(signal);
        await copySessionShareUrl(context, identity.shareId, () =>
          requireActiveContext(signal),
        );
        await onActivated();
      }),
    onSuccess: () => {
      trackAnalyticsEvent("share_link_copied", {
        entry_point: "share_panel",
      });
      sonnerToast.success(t`Share link copied.`);
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(t`Could not copy the share link.`);
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
      sx={styles.popover}
    >
      <AppFloatingPanel sx={styles.panel}>
        <div ref={operationLifecycleRef} {...stylex.props(styles.contents)}>
          <h2
            id="session-share-heading"
            {...stylex.props(styles.visuallyHidden)}
          >
            <Trans>Share</Trans>
          </h2>
          <p
            id="session-share-description"
            {...stylex.props(styles.visuallyHidden)}
          >
            <Trans>Invite people to this note.</Trans>
          </p>

          <div {...stylex.props(styles.scrollArea)}>
            <div {...stylex.props(styles.content)}>
              {error && !data ? (
                <div {...stylex.props(styles.loadError)}>
                  <p {...stylex.props(styles.loadErrorText)}>
                    <Trans>Access settings could not be loaded.</Trans>
                  </p>
                  <Button size="sm" variant="outline" onClick={onRetry}>
                    <ArrowsClockwise
                      {...stylex.props(styles.icon)}
                      aria-hidden="true"
                    />
                    <Trans>Try again</Trans>
                  </Button>
                </div>
              ) : null}
              {hasConflict ? (
                <section
                  aria-labelledby="sharing-conflict-heading"
                  {...stylex.props(styles.conflict)}
                >
                  <div {...stylex.props(styles.conflictRow)}>
                    <Warning
                      {...stylex.props(styles.warningIcon)}
                      aria-hidden="true"
                    />
                    <div {...stylex.props(styles.conflictContent)}>
                      <h3
                        id="sharing-conflict-heading"
                        {...stylex.props(styles.conflictHeading)}
                      >
                        <Trans>Sharing paused to protect your edits</Trans>
                      </h3>
                      <p {...stylex.props(styles.conflictDescription)}>
                        <Trans>
                          Resolve the web and desktop edits before inviting
                          anyone.
                        </Trans>
                      </p>
                      <div {...stylex.props(styles.conflictActions)}>
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          disabled={openWebCopyMutation.isPending}
                          onClick={() => openWebCopyMutation.mutate()}
                        >
                          {openWebCopyMutation.isPending ? (
                            <CircleNotch
                              {...stylex.props(styles.spinner)}
                              aria-hidden="true"
                            />
                          ) : (
                            <ArrowSquareOut
                              {...stylex.props(styles.icon)}
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
                              {...stylex.props(styles.spinner)}
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

              {recapMode === "invite" ? (
                <section aria-labelledby="invite-people-heading">
                  <h3
                    id="invite-people-heading"
                    {...stylex.props(styles.visuallyHidden)}
                  >
                    <Trans>Invite people</Trans>
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

                  <ShareInviteSuggestions
                    invite={invite}
                    disabled={!canPublish || inviteMutation.isPending}
                  />

                  <div {...stylex.props(styles.accessSection)}>
                    <h4 {...stylex.props(styles.accessHeading)}>
                      <Trans>People with access</Trans>
                    </h4>
                    <div {...stylex.props(styles.ownerRow)}>
                      <ContactFacehash name={ownerName} size={24} />
                      <div {...stylex.props(styles.ownerContent)}>
                        <p {...stylex.props(styles.ownerName)}>
                          {ownerName}{" "}
                          <span {...stylex.props(styles.muted)}>(You)</span>
                        </p>
                        {ownerEmail ? (
                          <p {...stylex.props(styles.ownerEmail)}>
                            {ownerEmail}
                          </p>
                        ) : null}
                      </div>
                      <span {...stylex.props(styles.accessLabel)}>
                        <Trans>Full access</Trans>
                      </span>
                    </div>

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
                  onBack={() => setRecapMode("invite")}
                  onSubmit={(emails) => emailMutation.mutate(emails)}
                />
              ) : (
                <SlackRecapForm
                  disabled={!canPublish || anyPending || deliveryPending}
                  pending={slackMutation.isPending}
                  onBack={() => setRecapMode("invite")}
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
            </div>
          </div>

          <footer {...stylex.props(styles.footer)}>
            <GeneralAccessSelector
              value={shownScopeValue}
              workspaces={workspaces}
              disabled={!management}
              canExpand={canPublish}
              pending={scopeMutation.isPending}
              allowedScopes={allowedScopes}
              onValueChange={(target) => {
                setOptimisticScope(target);
                scopeMutation.mutate(target);
              }}
            />
            <ShareRecapOverflowMenu onValueChange={setRecapMode} />
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
                generalCopyMutation.mutate();
              }}
              sx={styles.copyButton}
            >
              {generalCopyMutation.isPending || scopeMutation.isPending ? (
                <CircleNotch
                  {...stylex.props(styles.spinner)}
                  aria-hidden="true"
                />
              ) : (
                <Copy {...stylex.props(styles.icon)} aria-hidden="true" />
              )}
              <Trans>Copy link</Trans>
            </Button>
          </footer>
        </div>
      </AppFloatingPanel>
    </PopoverContent>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  accessHeading: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    fontWeight: 500,
    marginBottom: "0.25rem",
    paddingInline: "0.375rem",
  },
  accessLabel: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.6875rem",
  },
  accessSection: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    marginTop: "0.5rem",
    paddingTop: "0.5rem",
  },
  conflict: {
    backgroundColor: "rgb(245 158 11 / 0.1)",
    borderColor: "rgb(245 158 11 / 0.35)",
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    paddingBlock: "0.5rem",
    paddingInline: "0.625rem",
  },
  conflictActions: {
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
    marginTop: "0.625rem",
  },
  conflictContent: {
    flex: "1",
    minWidth: 0,
  },
  conflictDescription: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    lineHeight: "1rem",
    marginTop: "0.125rem",
  },
  conflictHeading: {
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  conflictRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.625rem",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  contents: {
    display: "contents",
  },
  copyButton: {
    borderRadius: radii.md,
    flexShrink: 0,
    fontSize: "0.75rem",
    height: "1.75rem",
    paddingInline: "0.625rem",
  },
  footer: {
    alignItems: "center",
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "flex",
    gap: "0.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  loadError: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.destructive} 5%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.destructive} 30%, transparent)`,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
    paddingBlock: "0.5rem",
    paddingInline: "0.625rem",
  },
  loadErrorText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  muted: {
    color: colors.mutedForeground,
  },
  ownerContent: {
    flex: "1",
    minWidth: 0,
  },
  ownerEmail: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ownerName: {
    fontSize: "0.75rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  ownerRow: {
    alignItems: "center",
    borderRadius: radii.lg,
    display: "flex",
    gap: "0.5rem",
    minHeight: "2.25rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.375rem",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    maxHeight: "min(530px, calc(100vh - 74px))",
    overflow: "hidden",
  },
  popover: {
    maxWidth: "calc(100vw - 16px)",
    overflow: "hidden",
    width: "440px",
  },
  scrollArea: {
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    overscrollBehavior: "contain",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    scrollbarColor:
      "hsl(var(--muted-foreground, 25 5% 45%) / 0.18) transparent",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  visuallyHidden: {
    borderWidth: 0,
    clipPath: "inset(50%)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
  warningIcon: {
    color: {
      default: "#d97706",
      "@media (prefers-color-scheme: dark)": "#fbbf24",
    },
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
});

export { styles as sessionShareManagementStyles };
