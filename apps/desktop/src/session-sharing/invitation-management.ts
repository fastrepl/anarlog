import { useForm } from "@tanstack/react-form";
import { useMutation } from "@tanstack/react-query";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  createSessionAccessInvitation,
  resendSessionAccessInvitation,
  sendSessionAccessInvitationEmail,
  type SessionAccessCapability,
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

export function useSessionInvitationManagement({
  identity,
  managementAvailable,
  canExpand,
  runOperation,
  publishLatest,
  requireActiveContext,
  onChanged,
}: {
  identity: SharePanelIdentity;
  managementAvailable: boolean;
  canExpand: boolean;
  runOperation: RunShareOperation;
  publishLatest: PublishLatestSessionShare;
  requireActiveContext: RequireActiveShareContext;
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

  return { inviteForm, inviteMutation };
}
