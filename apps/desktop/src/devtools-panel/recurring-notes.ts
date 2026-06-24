import type {
  EnhancedNoteStorage,
  HumanStorage,
  MappingSessionParticipantStorage,
  SessionEvent,
  SessionKeyFactsStorage,
  SessionStorage,
} from "@hypr/store";

import { buildPastSessionNotes } from "~/session/components/bottom-accessory/past-notes";
import { DEFAULT_USER_ID } from "~/shared/utils";
import type * as main from "~/store/tinybase/store/main";

type Store = main.Store;

const CURRENT_SESSION_ID = "devtools-recurring-notes-current";
const SERIES_ID = "devtools-recurring-product-sync";
const CALENDAR_ID = "devtools-calendar";
const MEETING_TITLE = "Devtools Weekly Product Sync";
const DAY_MS = 24 * 60 * 60 * 1000;

const PARTICIPANTS = [
  {
    humanId: "devtools-human-alex-rivera",
    name: "Alex Rivera",
    email: "alex.rivera@example.com",
    jobTitle: "Product Lead",
  },
  {
    humanId: "devtools-human-maya-chen",
    name: "Maya Chen",
    email: "maya.chen@example.com",
    jobTitle: "Design Lead",
  },
  {
    humanId: "devtools-human-jordan-lee",
    name: "Jordan Lee",
    email: "jordan.lee@example.com",
    jobTitle: "Engineering Lead",
  },
] as const;

const PAST_NOTES = [
  {
    sessionId: "devtools-recurring-notes-week-1",
    daysAgo: 7,
    rawMd: [
      "# Product sync",
      "- Shipped the condensed transcript panel and agreed to keep Past Notes below three visible lines per fact.",
      "- Alex owns the launch checklist and will confirm analytics events before the next review.",
      "- Maya wants another pass on empty states after the first beta feedback lands.",
    ].join("\n"),
    facts: [
      "Transcript controls shipped with a condensed panel layout.",
      "Alex owns the launch checklist and analytics confirmation.",
      "Maya wants another empty-state pass after beta feedback.",
    ],
  },
  {
    sessionId: "devtools-recurring-notes-week-2",
    daysAgo: 14,
    rawMd: [
      "# Product sync",
      "- The team decided Past Notes should match by recurring calendar series before falling back to participants.",
      "- Jordan called out that cached key facts should avoid requiring a model just to inspect the UI.",
      "- Follow-up: compare date labels against the meeting start time instead of the note creation time.",
    ].join("\n"),
    facts: [
      "Past Notes should prefer recurring series matches before participant fallback.",
      "Cached key facts should make the UI inspectable without a model.",
      "Date labels should come from meeting start time.",
    ],
  },
  {
    sessionId: "devtools-recurring-notes-week-3",
    daysAgo: 21,
    rawMd: [
      "# Product sync",
      "- We agreed the bottom accessory should stay hidden until there is useful post-session content.",
      "- The first version of Past Notes will stay read-only and focus on short reusable facts.",
      "- Alex and Jordan will validate that future sessions are excluded from the timeline.",
    ].join("\n"),
    facts: [
      "The bottom accessory should stay hidden without useful post-session content.",
      "Past Notes will start as a read-only timeline of reusable facts.",
      "Future sessions should be excluded from the Past Notes timeline.",
    ],
  },
] as const;

export function populateRecurringMeetingNotes({
  store,
  userId,
  now = new Date(),
}: {
  store: Store;
  userId: string | null | undefined;
  now?: Date;
}): string {
  const ownerUserId = normalizeUserId(userId);
  const createdAt = now.toISOString();

  store.transaction(() => {
    for (const participant of PARTICIPANTS) {
      store.setRow("humans", participant.humanId, {
        user_id: ownerUserId,
        created_at: createdAt,
        name: participant.name,
        email: participant.email,
        phone: "",
        org_id: "",
        job_title: participant.jobTitle,
        linkedin_username: "",
        memo: "",
        pinned: false,
      } satisfies HumanStorage);
    }

    upsertSession({
      store,
      ownerUserId,
      sessionId: CURRENT_SESSION_ID,
      startedAt: now,
      rawMd:
        "Use the Past notes tab to inspect the cached timeline from previous occurrences.",
    });

    for (const note of PAST_NOTES) {
      upsertSession({
        store,
        ownerUserId,
        sessionId: note.sessionId,
        startedAt: new Date(now.getTime() - note.daysAgo * DAY_MS),
        rawMd: note.rawMd,
      });
    }
  });

  const factsBySessionId = new Map<string, string>(
    PAST_NOTES.map((note) => [note.sessionId, note.facts.join("\n")]),
  );
  const { missing } = buildPastSessionNotes(
    store,
    CURRENT_SESSION_ID,
    ownerUserId,
  );

  store.transaction(() => {
    for (const request of missing) {
      const facts = factsBySessionId.get(request.sessionId);
      if (!facts) {
        continue;
      }

      store.setRow("session_key_facts", request.sessionId, {
        user_id: ownerUserId,
        session_id: request.sessionId,
        created_at: createdAt,
        updated_at: createdAt,
        content: facts,
        source_hash: request.sourceHash,
      } satisfies SessionKeyFactsStorage);
    }
  });

  return CURRENT_SESSION_ID;
}

function upsertSession({
  store,
  ownerUserId,
  sessionId,
  startedAt,
  rawMd,
}: {
  store: Store;
  ownerUserId: string;
  sessionId: string;
  startedAt: Date;
  rawMd: string;
}) {
  const endedAt = new Date(startedAt.getTime() + 45 * 60 * 1000);
  const event: SessionEvent = {
    tracking_id: `${SERIES_ID}:${toDateId(startedAt)}`,
    calendar_id: CALENDAR_ID,
    title: MEETING_TITLE,
    started_at: startedAt.toISOString(),
    ended_at: endedAt.toISOString(),
    is_all_day: false,
    has_recurrence_rules: true,
    meeting_link: "https://zoom.us/j/1234567890",
    description: "Seeded from devtools to exercise the Past Notes tab.",
    recurrence_series_id: SERIES_ID,
  };

  store.setRow("sessions", sessionId, {
    user_id: ownerUserId,
    created_at: startedAt.toISOString(),
    event_json: JSON.stringify(event),
    title: MEETING_TITLE,
    raw_md: rawMd,
  } satisfies SessionStorage);

  store.setRow("enhanced_notes", `${sessionId}:summary`, {
    user_id: ownerUserId,
    session_id: sessionId,
    content: rawMd,
    template_id: "",
    position: 0,
    title: "Summary",
  } satisfies EnhancedNoteStorage);

  for (const participant of PARTICIPANTS) {
    store.setRow(
      "mapping_session_participant",
      `${sessionId}:${participant.humanId}`,
      {
        user_id: ownerUserId,
        session_id: sessionId,
        human_id: participant.humanId,
        source: "auto",
      } satisfies MappingSessionParticipantStorage,
    );
  }
}

function normalizeUserId(userId: string | null | undefined): string {
  return userId?.trim() || DEFAULT_USER_ID;
}

function toDateId(date: Date): string {
  return date.toISOString().slice(0, 10);
}
