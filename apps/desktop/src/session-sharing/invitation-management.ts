import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  createSessionAccessInvitation,
  resendSessionAccessInvitation,
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
  signal,
  requireActive,
}: {
  context: ShareManagementContext;
  shareId: string;
  email: string;
  capability: SessionAccessCapability;
  noteTitle: string;
  signal: AbortSignal;
  requireActive: () => void;
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
      signal,
    });
  } catch {
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

  const inviteMutation = useMutation({
    mutationFn: (input: {
      email: string;
      capability: SessionAccessCapability;
    }) =>
      runOperation(async (signal) => {
        if (!canExpand || !managementAvailable) {
          throw new ShareManagementError();
        }
        const published = await publishLatest(signal);
        const context = requireActiveContext(signal);
        const delivery = await deliverSessionShareInvitation({
          context,
          shareId: identity.shareId,
          email: input.email,
          capability: input.capability,
          noteTitle: published.title,
          signal,
          requireActive: () => {
            requireActiveContext(signal);
          },
        });
        await onActivated();
        return delivery;
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

  return { inviteForm, inviteMutation };
}
