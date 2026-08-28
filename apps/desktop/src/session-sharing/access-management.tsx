import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  createSessionAccessInvitation,
  listSessionShareAccess,
  resendSessionAccessInvitation,
  reviewSessionAccessRequest,
  revokeSessionAccessGrant,
  revokeSessionAccessInvitation,
  sendSessionAccessInvitationEmail,
  type SessionAccessCapability,
  type SessionShareAccessEntry,
  type SessionShareManagement,
  ShareManagementError,
  updateSessionAccessGrant,
} from "./client";
import { getSessionShareSenderName } from "./invitation-management";
import {
  copyInvitationOrRevoke,
  ShareOperationAbortedError,
  type SharePanelData,
  type SharePanelIdentity,
  withoutSignal,
} from "./management";
import {
  type PublishLatestSessionShare,
  type RequireActiveShareContext,
  type RunShareOperation,
} from "./management-operation";

import { ContactFacehash } from "~/contacts/shared";
import { env } from "~/env";

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

const capabilityRanks: Record<SessionAccessCapability, number> = {
  viewer: 1,
  commenter: 2,
  editor: 3,
};

export function useSessionAccessManagement({
  identity,
  data,
  management,
  canExpand,
  runOperation,
  publishLatest,
  requireActiveContext,
  onChanged,
}: {
  identity: SharePanelIdentity;
  data: SharePanelData | undefined;
  management: SessionShareManagement | undefined;
  canExpand: boolean;
  runOperation: RunShareOperation;
  publishLatest: PublishLatestSessionShare;
  requireActiveContext: RequireActiveShareContext;
  onChanged: () => Promise<unknown>;
}) {
  return useMutation({
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
              senderName: getSessionShareSenderName(context.session.user),
              signal,
            });
          } catch {
            await copyInvitationOrRevoke(
              withoutSignal(context),
              identity.shareId,
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
              senderName: getSessionShareSenderName(context.session.user),
              signal,
            });
          } catch {
            await copyInvitationOrRevoke(
              withoutSignal(context),
              identity.shareId,
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
          ? t`Invitation sent.`
          : deliveredBy === "clipboard"
            ? t`Email unavailable. Invite link copied instead.`
            : t`Access updated.`,
      );
    },
    onError: (error) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(t`Could not update this person's access.`);
    },
    onSettled: onChanged,
  });
}

export function AccessEntryRow({
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
  const label = contactName || entry.userEmail || t`Anarlog user`;
  const capabilityLabels: Record<SessionAccessCapability, string> = {
    viewer: t`Can view`,
    commenter: t`Can comment`,
    editor: t`Can edit`,
  };

  return (
    <div {...stylex.props(styles.row)}>
      <ContactFacehash name={label} size={24} />
      <div {...stylex.props(styles.content)}>
        <p {...stylex.props(styles.name)}>{label}</p>
        <p {...stylex.props(styles.description)}>
          {contactName && entry.userEmail
            ? entry.userEmail
            : entry.entryType === "grant"
              ? t`Anarlog member`
              : entry.entryType === "invitation"
                ? t`Invitation pending`
                : t`Requested ${capabilityLabels[entry.capability].toLowerCase()}`}
        </p>
      </div>
      {pending ? (
        <CircleNotch
          {...stylex.props(styles.spinner)}
          aria-label={t`Updating access`}
        />
      ) : entry.entryType === "request" ? (
        <div {...stylex.props(styles.requestActions)}>
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
        <CapabilitySelect
          value={entry.capability}
          disabled={!canExpand && entry.entryType === "invitation"}
          ariaLabel={t`Permission for ${label}`}
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
      <SelectTrigger aria-label={ariaLabel} sx={styles.capabilityTrigger}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem
          value="viewer"
          disabled={capabilityRanks.viewer > maximumRank}
        >
          {t`Can view`}
        </SelectItem>
        <SelectItem
          value="commenter"
          disabled={capabilityRanks.commenter > maximumRank}
        >
          {t`Can comment`}
        </SelectItem>
        <SelectItem
          value="editor"
          disabled={capabilityRanks.editor > maximumRank}
        >
          {t`Can edit`}
        </SelectItem>
        {onResend || onRemove ? <SelectSeparator /> : null}
        {onResend ? (
          <SelectItem value="resend">{t`Resend invite`}</SelectItem>
        ) : null}
        {onRemove ? (
          <SelectItem value="remove" sx={styles.removeItem}>
            {t`Remove`}
          </SelectItem>
        ) : null}
      </SelectContent>
    </Select>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  capabilityTrigger: {
    backgroundColor: "transparent",
    borderRadius: radii.md,
    borderWidth: 0,
    boxShadow: "none",
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.6875rem",
    gap: "0.25rem",
    height: "1.75rem",
    minWidth: "5.25rem",
    paddingInline: "0.375rem",
    width: "auto",
  },
  content: {
    flex: "1",
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  name: {
    fontSize: "0.75rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  removeItem: {
    color: colors.destructive,
  },
  requestActions: {
    alignItems: "center",
    display: "flex",
    gap: "0.25rem",
  },
  row: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.accent} 50%, transparent)`,
    },
    borderRadius: radii.lg,
    display: "flex",
    gap: "0.5rem",
    minHeight: "2.25rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.375rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
});
