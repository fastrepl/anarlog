import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { WELCOME_NOTE_TRACKING_ID } from "./welcome-note.constants";

import { loadSessionContentSnapshot } from "~/session/content-queries";

export function getMeetingNoteCompletionEvent(event: unknown) {
  if (
    event &&
    typeof event === "object" &&
    "tracking_id" in event &&
    event.tracking_id === WELCOME_NOTE_TRACKING_ID
  ) {
    return "welcome_meeting_note_completed";
  }

  return "meeting_note_completed";
}

export async function trackMeetingNoteCompletion(sessionId: string) {
  const snapshot = await loadSessionContentSnapshot(sessionId);
  if (!snapshot) {
    return;
  }

  await analyticsCommands.eventFireAndForget({
    event: getMeetingNoteCompletionEvent(snapshot.event),
  });
}
