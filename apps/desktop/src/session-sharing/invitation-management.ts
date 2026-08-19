import { t } from "@lingui/core/macro";
import { useMutation } from "@tanstack/react-query";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  createSessionAccessInvitation,
  resendSessionAccessInvitation,
  revokeSessionAccessInvitation,
  sendSessionAccessInvitationEmail,
  type SessionAccessCapability,
  type ShareManagementContext,
  ShareManagementError,
} from "./client";
import {
  copyInvitationOrRevoke,
  ShareOperationAbortedError,
  type SharePanelIdentity,
  withoutSignal,
} from "./management";
import {
  type PublishLatestSessionShare,
  type RequireActiveShareContext,
  type RunShareOperation,
} from "./management-operation";

import { trackAnalyticsEvent } from "~/analytics";
import { env } from "~/env";

export function isInviteEmail(value: string) {
  const normalized = value.trim();
  return (
    normalized.length <= 320 &&
    /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(normalized) &&
    !/[\u0000-\u001f\u007f]/.test(normalized)
  );
}

export async function deliverSessionShareInvitation({
  context,
  shareId,
  email,
  capability,
  noteTitle,
  senderName,
  signal,
  requireActive,
  allowClipboardFallback,
}: {
  context: ShareManagementContext;
  shareId: string;
  email: string;
  capability: SessionAccessCapability;
  noteTitle: string;
  senderName: string;
  signal: AbortSignal;
  requireActive: () => void;
  allowClipboardFallback: boolean;
}) {
  let invitation = await createSessionAccessInvitation(context, {
    shareId,
    inviteeEmail: email,
    capability,
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
      shareId,
      invitationId: invitation.invitationId,
      inviteToken: invitation.inviteToken,
      noteTitle,
      senderName,
      signal,
    });
  } catch {
    requireActive();
    if (!allowClipboardFallback) {
      await revokeSessionAccessInvitation(
        withoutSignal(context),
        invitation.invitationId,
      ).catch(() => undefined);
      throw new ShareManagementError();
    }
    await copyInvitationOrRevoke(
      withoutSignal(context),
      {
        invitationId: invitation.invitationId,
        inviteToken: invitation.inviteToken,
      },
      requireActive,
      signal,
    );
    return { deliveredBy: "clipboard" as const };
  }
  requireActive();
  return { deliveredBy: "email" as const };
}

export type SessionShareInvitationDelivery = {
  email: string;
  deliveredBy: "email" | "clipboard" | null;
};

export async function deliverSessionShareInvitations({
  emails,
  ...invitation
}: {
  context: ShareManagementContext;
  shareId: string;
  emails: string[];
  capability: SessionAccessCapability;
  noteTitle: string;
  senderName: string;
  signal: AbortSignal;
  requireActive: () => void;
}): Promise<SessionShareInvitationDelivery[]> {
  const deliveries: SessionShareInvitationDelivery[] = [];
  const allowClipboardFallback = emails.length === 1;
  for (const email of emails) {
    invitation.requireActive();
    try {
      const { deliveredBy } = await deliverSessionShareInvitation({
        ...invitation,
        email,
        allowClipboardFallback,
      });
      deliveries.push({ email, deliveredBy });
    } catch (error) {
      if (invitation.signal.aborted) throw error;
      console.error("[session-sharing] could not invite", email, error);
      deliveries.push({ email, deliveredBy: null });
    }
  }
  return deliveries;
}

export function reportSessionShareInvitations(
  deliveries: SessionShareInvitationDelivery[],
  capability: SessionAccessCapability,
) {
  const sent = deliveries.filter((delivery) => delivery.deliveredBy);
  for (const delivery of sent) {
    trackAnalyticsEvent("share_invitation_sent", {
      delivery_method: delivery.deliveredBy,
      capability,
    });
  }
  const failed = deliveries.length - sent.length;
  if (!sent.length) {
    sonnerToast.error(
      deliveries.length > 1
        ? t`Could not create these invitations.`
        : t`Could not create this invitation.`,
    );
    return;
  }
  if (failed) {
    sonnerToast.error(
      t`Invited ${sent.length}. Could not invite ${failed}. Try again.`,
    );
    return;
  }
  sonnerToast.success(
    sent.some((delivery) => delivery.deliveredBy === "clipboard")
      ? t`Email unavailable. Invite link copied instead.`
      : sent.length > 1
        ? t`Invitations sent.`
        : t`Invitation sent.`,
  );
}

export function useSessionInvitationManagement({
  identity,
  managementAvailable,
  canExpand,
  runOperation,
  publishLatest,
  requireActiveContext,
  onActivated,
  onChanged,
}: {
  identity: SharePanelIdentity;
  managementAvailable: boolean;
  canExpand: boolean;
  runOperation: RunShareOperation;
  publishLatest: PublishLatestSessionShare;
  requireActiveContext: RequireActiveShareContext;
  onActivated: () => Promise<unknown>;
  onChanged: () => Promise<unknown>;
}) {
  const inviteMutation = useMutation({
    mutationFn: (input: {
      emails: string[];
      capability: SessionAccessCapability;
    }) =>
      runOperation(async (signal) => {
        if (!canExpand || !managementAvailable) {
          throw new ShareManagementError();
        }
        const published = await publishLatest(signal);
        const context = requireActiveContext(signal);
        const deliveries = await deliverSessionShareInvitations({
          context,
          shareId: identity.shareId,
          emails: input.emails,
          capability: input.capability,
          noteTitle: published.title,
          senderName: getSessionShareSenderName(context.session.user),
          signal,
          requireActive: () => {
            requireActiveContext(signal);
          },
        });
        await onActivated();
        return deliveries;
      }),
    onSuccess: (deliveries, input) => {
      reportSessionShareInvitations(deliveries, input.capability);
    },
    onError: (error, input) => {
      if (error instanceof ShareOperationAbortedError) return;
      sonnerToast.error(
        input.emails.length > 1
          ? "Could not create these invitations."
          : "Could not create this invitation.",
      );
    },
    onSettled: onChanged,
  });

  return { inviteMutation };
}

export function getSessionShareSenderName(user: {
  email?: string;
  user_metadata?: Record<string, unknown>;
}) {
  const metadata = user.user_metadata;
  return typeof metadata?.full_name === "string" && metadata.full_name.trim()
    ? metadata.full_name.trim()
    : typeof metadata?.name === "string" && metadata.name.trim()
      ? metadata.name.trim()
      : user.email || "An Anarlog user";
}
