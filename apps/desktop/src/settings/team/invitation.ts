import { t } from "@lingui/core/macro";
import { isTauri } from "@tauri-apps/api/core";
import { writeText as writeClipboardText } from "@tauri-apps/plugin-clipboard-manager";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  createWorkspaceInvitation,
  resendWorkspaceInvitation,
  revokeInvitation,
  sendWorkspaceInvitationEmail,
  type TeamContext,
  TeamError,
} from "./client";

import { env } from "~/env";

export async function deliverWorkspaceInvitation({
  context,
  workspaceId,
  workspaceName,
  email,
  senderName,
}: {
  context: TeamContext;
  workspaceId: string;
  workspaceName: string;
  email: string;
  senderName: string;
}) {
  let invitation = await createWorkspaceInvitation(context, workspaceId, email);
  if (!invitation.inviteToken) {
    invitation = await resendWorkspaceInvitation(
      context,
      invitation.invitationId,
    );
  }
  const inviteToken = invitation.inviteToken;
  if (!inviteToken) throw new TeamError();
  const created = {
    invitationId: invitation.invitationId,
    inviteToken,
  };
  try {
    await sendWorkspaceInvitationEmail({
      apiBaseUrl: env.VITE_API_URL,
      accessToken: context.session.access_token,
      workspaceId,
      invitationId: created.invitationId,
      inviteToken: created.inviteToken,
      workspaceName,
      senderName,
    });
    return { deliveredBy: "email" as const };
  } catch {
    try {
      await copyText(buildTeamInvitationUrl(created));
      return { deliveredBy: "clipboard" as const };
    } catch {
      await revokeInvitation(context, created.invitationId).catch(
        () => undefined,
      );
      throw new TeamError();
    }
  }
}

export function reportWorkspaceInvitation(deliveredBy: "email" | "clipboard") {
  sonnerToast.success(
    deliveredBy === "clipboard"
      ? t`Email unavailable. Invite link copied instead.`
      : t`Invitation sent.`,
  );
}

export function getTeamSenderName(user: {
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

function buildTeamInvitationUrl(invitation: {
  invitationId: string;
  inviteToken: string;
}) {
  const base = new URL(env.VITE_APP_URL);
  if (
    !["http:", "https:"].includes(base.protocol) ||
    base.username !== "" ||
    base.password !== "" ||
    base.search !== "" ||
    base.hash !== ""
  ) {
    throw new TeamError();
  }
  const url = new URL(`/team/invite/${invitation.invitationId}/`, base.origin);
  url.hash = new URLSearchParams({ token: invitation.inviteToken }).toString();
  return url.toString();
}

async function copyText(value: string) {
  if (isTauri()) {
    await writeClipboardText(value);
    return;
  }
  await navigator.clipboard.writeText(value);
}
