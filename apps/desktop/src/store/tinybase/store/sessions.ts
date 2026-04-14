import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type {
  EventParticipant,
  HumanStorage,
  MappingSessionParticipantStorage,
  SessionEvent,
} from "@hypr/store";
import { json2md } from "@hypr/tiptap/shared";

import * as main from "./main";

import { eventExists, getEventById } from "~/calendar/queries";
import { findSessionByEventId } from "~/session/utils";
import { DEFAULT_USER_ID } from "~/shared/utils";
import { id } from "~/shared/utils";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

export function createSession(store: Store, title?: string): string {
  const sessionId = id();
  store.setRow("sessions", sessionId, {
    title: title ?? "",
    created_at: new Date().toISOString(),
    raw_md: "",
    user_id: DEFAULT_USER_ID,
  });
  void analyticsCommands.event({
    event: "note_created",
    has_event_id: false,
  });
  return sessionId;
}

export async function getOrCreateSessionForEventId(
  store: Store,
  eventId: string,
  title?: string,
): Promise<string> {
  if (!(await eventExists(eventId))) {
    console.trace(
      `[getOrCreateSessionForEventId] event that corresponds to the provided eventId ${eventId} does not exist`,
    );
    return createSession(store, title);
  }

  const existingSessionId = await findSessionByEventId(store, eventId);
  if (existingSessionId) {
    return existingSessionId;
  }

  const event = await getEventById(eventId);
  if (!event) {
    return createSession(store, title);
  }

  let sessionEvent: SessionEvent = {
    tracking_id: event.trackingIdEvent,
    calendar_id: event.calendarId,
    title: event.title,
    started_at: event.startedAt,
    ended_at: event.endedAt,
    is_all_day: event.isAllDay,
    has_recurrence_rules: event.hasRecurrenceRules,
    location: event.location,
    meeting_link: event.meetingLink,
    description: event.description,
    recurrence_series_id: event.recurrenceSeriesId,
  };

  const sessionId = id();
  store.setRow("sessions", sessionId, {
    event_json: JSON.stringify(sessionEvent),
    title: title ?? sessionEvent.title,
    created_at: new Date().toISOString(),
    raw_md: "",
    user_id: DEFAULT_USER_ID,
  });

  createParticipantsFromEvent(store, sessionId, event);

  void analyticsCommands.event({
    event: "note_created",
    has_event_id: true,
  });
  return sessionId;
}

export function isSessionEmpty(store: Store, sessionId: string): boolean {
  const session = store.getRow("sessions", sessionId);
  if (!session) {
    return true;
  }

  // event sessions automatically have a title
  // only consider titles if it does not have an event
  if (session.title && session.title.trim() && !session.event_json) {
    return false;
  }

  if (session.raw_md) {
    let raw_md: string;
    try {
      raw_md = json2md(JSON.parse(session.raw_md));
    } catch {
      raw_md = session.raw_md;
    }
    raw_md = raw_md.trim();
    // see: https://github.com/ueberdosis/tiptap/issues/7495
    // this is a known regression on @tiptap/markdown on v3.18.0.
    if (raw_md && raw_md !== "&nbsp;") {
      return false;
    }
  }

  let hasTranscript = false;
  store.forEachRow("transcripts", (rowId, _forEachCell) => {
    const row = store.getRow("transcripts", rowId);
    if (row?.session_id === sessionId) {
      hasTranscript = true;
    }
  });
  if (hasTranscript) {
    return false;
  }

  let hasEnhancedNote = false;
  store.forEachRow("enhanced_notes", (rowId, _forEachCell) => {
    const row = store.getRow("enhanced_notes", rowId);
    if (row?.session_id === sessionId) {
      hasEnhancedNote = true;
    }
  });
  if (hasEnhancedNote) {
    return false;
  }

  let hasManualParticipant = false;
  store.forEachRow("mapping_session_participant", (rowId, _forEachCell) => {
    const row = store.getRow("mapping_session_participant", rowId);
    if (row?.session_id === sessionId && row.source !== "auto") {
      hasManualParticipant = true;
    }
  });
  if (hasManualParticipant) {
    return false;
  }

  let hasTag = false;
  store.forEachRow("mapping_tag_session", (rowId, _forEachCell) => {
    const row = store.getRow("mapping_tag_session", rowId);
    if (row?.session_id === sessionId) {
      hasTag = true;
    }
  });
  if (hasTag) {
    return false;
  }

  return true;
}

function createParticipantsFromEvent(
  store: Store,
  sessionId: string,
  event: { participantsJson: unknown },
): void {
  if (!event.participantsJson) return;

  let participants: EventParticipant[];
  try {
    const raw =
      typeof event.participantsJson === "string"
        ? event.participantsJson
        : JSON.stringify(event.participantsJson);
    participants = JSON.parse(raw);
  } catch {
    return;
  }

  if (!Array.isArray(participants) || participants.length === 0) return;

  const humansByEmail = new Map<string, string>();
  store.forEachRow("humans", (humanId, _forEachCell) => {
    const human = store.getRow("humans", humanId);
    const email = human?.email;
    if (email && typeof email === "string" && email.trim()) {
      humansByEmail.set(email.toLowerCase(), humanId);
    }
  });

  for (const participant of participants) {
    if (!participant.email) continue;

    const emailLower = participant.email.toLowerCase();
    let humanId = humansByEmail.get(emailLower);

    if (!humanId) {
      humanId = id();
      store.setRow("humans", humanId, {
        user_id: DEFAULT_USER_ID,
        name: participant.name || participant.email,
        email: participant.email,
        org_id: "",
        job_title: "",
        linkedin_username: "",
        memo: "",
        pinned: false,
      } satisfies HumanStorage);
      humansByEmail.set(emailLower, humanId);
    }

    store.setRow("mapping_session_participant", id(), {
      user_id: DEFAULT_USER_ID,
      session_id: sessionId,
      human_id: humanId,
      source: "auto",
    } satisfies MappingSessionParticipantStorage);
  }
}
