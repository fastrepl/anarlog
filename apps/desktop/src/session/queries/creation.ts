import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands } from "@anlg/plugin-session";
import { eventParticipantSchema, type EventParticipant } from "@anlg/store";

import { updateSession } from "./sessions";
import type { SessionChanges } from "./types";

import { getSeriesFolderRule } from "~/calendar/series-folders";
import { deriveContactIdentity } from "~/contacts/identity";
import { liveQueryClient } from "~/db";
import { ensureFolderCatalog } from "~/session/folder-catalog";
import { normalizeFolderPath } from "~/session/folders";
import { DEFAULT_USER_ID } from "~/shared/utils";

type EventParticipantsSqlRow = {
  id: string;
  participants_json: string | null;
  recurrence_series_id: string | null;
};

export async function createSession(
  title = "",
  userId = DEFAULT_USER_ID,
  initial?: Pick<SessionChanges, "event_json" | "folder_id" | "raw_md">,
): Promise<string> {
  const folderPath = normalizeFolderPath(initial?.folder_id ?? "") ?? "";

  const result = await commands.createSession({
    title,
    user_id: userId,
    event_json: initial?.event_json ?? "",
    folder_path: folderPath,
    raw_md: initial?.raw_md ?? "",
  });
  if (result.status === "error") throw new Error(result.error);
  const sessionId = result.data;

  if (folderPath) {
    await ensureFolderCatalog(folderPath);
  }

  trackNoteCreated(false);
  return sessionId;
}

export async function getOrCreateSessionForEventId(
  eventId: string,
  title?: string,
  userId = DEFAULT_USER_ID,
): Promise<string> {
  const [event] = await liveQueryClient.execute<EventParticipantsSqlRow>(
    `
      SELECT id, participants_json, recurrence_series_id
      FROM events
      WHERE id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    [eventId],
  );

  if (!event) {
    return createSession(title, userId);
  }

  const result = await commands.createSessionForEvent({
    event_id: eventId,
    user_id: userId,
    title: title ?? null,
    participants: resolveEventParticipantIdentities(
      parseEventParticipants(event.participants_json),
    ),
  });
  if (result.status === "error") throw new Error(result.error);
  if (!result.data) {
    return createSession(title, userId);
  }

  if (result.data.created) {
    trackNoteCreated(true);
    await applySeriesFolderRule(
      result.data.session_id,
      event.recurrence_series_id,
    );
  }
  return result.data.session_id;
}

async function applySeriesFolderRule(
  sessionId: string,
  seriesId: string | null,
): Promise<void> {
  if (!seriesId) return;
  try {
    const folderPath = await getSeriesFolderRule(seriesId);
    if (!folderPath) return;
    const [folder] = await liveQueryClient.execute<{ present: number }>(
      `
        SELECT 1 AS present
        FROM folders
        WHERE path = ? AND deleted_at IS NULL
        LIMIT 1
      `,
      [folderPath],
    );
    if (!folder) return;
    await updateSession(sessionId, { folder_id: folderPath });
  } catch (error) {
    console.error("[session] failed to apply series folder rule", error);
  }
}

function resolveEventParticipantIdentities(
  participants: EventParticipant[],
): Array<{ email: string; name: string; company_name: string | null }> {
  const seenEmails = new Set<string>();
  const identities: Array<{
    email: string;
    name: string;
    company_name: string | null;
  }> = [];

  for (const participant of participants) {
    if (participant.is_current_user === true) continue;
    const email = participant.email?.trim();
    if (!email) continue;
    const emailKey = email.toLowerCase();
    if (seenEmails.has(emailKey)) continue;
    seenEmails.add(emailKey);

    const identity = deriveContactIdentity({ name: participant.name, email });
    identities.push({
      email,
      name: identity.name,
      company_name: identity.companyName ?? null,
    });
  }

  return identities;
}

function parseEventParticipants(value: string | null): EventParticipant[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed)
      ? parsed.flatMap((participant) => {
          const result = eventParticipantSchema.safeParse(participant);
          return result.success ? [result.data] : [];
        })
      : [];
  } catch {
    return [];
  }
}

function trackNoteCreated(hasEventId: boolean): void {
  void analyticsCommands
    .eventFireAndForget({
      event: "note_created",
      has_event_id: hasEventId,
    })
    .catch((error) => {
      console.error(
        "[session] failed to record note creation analytics",
        error,
      );
    });
}
