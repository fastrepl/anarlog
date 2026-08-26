import { setSessionShareScope } from "./client";
import type { ShareManagementContext } from "./client-contract";
import type { GeneralAccessTarget } from "./general-access";
import {
  deliverSessionShareInvitations,
  getSessionShareSenderName,
  isInviteEmail,
} from "./invitation-management";
import type { AvailableShareWorkspace } from "./source";

import { liveQueryClient } from "~/db";

export const DEFAULT_MEETING_SHARE_ACCESS_VALUES = [
  "me",
  "participants",
  "workspace",
] as const;

export type DefaultMeetingShareAccess =
  (typeof DEFAULT_MEETING_SHARE_ACCESS_VALUES)[number];

type ParticipantEmailSqlRow = { email: string };

export function normalizeDefaultMeetingShareAccess(
  value: string | undefined,
): DefaultMeetingShareAccess {
  return DEFAULT_MEETING_SHARE_ACCESS_VALUES.includes(
    value as DefaultMeetingShareAccess,
  )
    ? (value as DefaultMeetingShareAccess)
    : "me";
}

export function defaultGeneralAccessTarget(
  access: DefaultMeetingShareAccess,
  workspaces: AvailableShareWorkspace[],
): GeneralAccessTarget {
  if (access === "workspace") {
    const workspaceId = workspaces[0]?.id;
    if (workspaceId) return `workspace:${workspaceId}`;
  }
  return "restricted";
}

export async function loadMeetingShareInviteEmails({
  sessionId,
  ownerEmail,
}: {
  sessionId: string;
  ownerEmail: string;
}): Promise<string[]> {
  const rows = await liveQueryClient.execute<ParticipantEmailSqlRow>(
    `
      SELECT COALESCE(NULLIF(human.email, ''), participant.email) AS email
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id
        AND human.deleted_at IS NULL
      WHERE participant.session_id = ?
        AND participant.source <> 'excluded'
        AND participant.deleted_at IS NULL
      ORDER BY participant.created_at, participant.id
    `,
    [sessionId],
  );

  const excluded = new Set(
    [ownerEmail].map((email) => email.trim().toLowerCase()).filter(Boolean),
  );
  const emails: string[] = [];
  for (const row of rows) {
    const email = row.email.trim();
    const key = email.toLowerCase();
    if (!isInviteEmail(email) || excluded.has(key)) continue;
    excluded.add(key);
    emails.push(email);
  }
  return emails;
}

export async function applyDefaultMeetingShareAccess(input: {
  wasCreated: boolean;
  actionType: "invite" | "email" | "slack" | "copy-link" | "scope";
  access: DefaultMeetingShareAccess;
  workspaces: AvailableShareWorkspace[];
  context: ShareManagementContext;
  shareId: string;
  sessionId: string;
  noteTitle: string;
  signal: AbortSignal;
  requireActive: () => void;
}): Promise<void> {
  if (!input.wasCreated || input.actionType === "scope") return;

  if (input.access === "workspace") {
    const workspaceId = input.workspaces[0]?.id;
    if (!workspaceId) return;
    input.requireActive();
    try {
      await setSessionShareScope(input.context, {
        shareId: input.shareId,
        scope: "workspace",
        workspaceId,
      });
    } catch (error) {
      console.error(
        "[session-sharing] could not apply workspace default access",
        error,
      );
    }
    return;
  }

  if (input.access !== "participants" || input.actionType === "invite") {
    return;
  }

  const emails = await loadMeetingShareInviteEmails({
    sessionId: input.sessionId,
    ownerEmail: input.context.session.user.email ?? "",
  });
  if (emails.length === 0) return;

  input.requireActive();
  await deliverSessionShareInvitations({
    context: input.context,
    shareId: input.shareId,
    emails,
    capability: "viewer",
    noteTitle: input.noteTitle,
    senderName: getSessionShareSenderName(input.context.session.user),
    signal: input.signal,
    requireActive: input.requireActive,
  });
}
