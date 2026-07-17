import { md2json } from "@hypr/editor/markdown";
import type { SessionEvent } from "@hypr/store";

import { liveQueryClient } from "~/db";
import { createSession } from "~/session/queries";
import { DEFAULT_USER_ID } from "~/shared/utils";

const DEMO_URL = "https://anarlog.so/onboarding-demo/";
const PENDING_WELCOME_SESSION_KEY = "anarlog.pending-welcome-session";
const TRACKING_ID = "anarlog-onboarding-demo-v1";

const WELCOME_NOTE = `Welcome to Anarlog 👋

This note is a quick way to see how Anarlog works.

Click **Join & record** in the top-right corner. It will open a private, prerecorded demo meeting, so you don't have to worry about your camera or microphone. Anarlog will listen, transcribe the conversation, and turn it into notes just like a real meeting.

When the video ends, come back here to review the transcript and notes.`;

let pendingWelcomeSession: Promise<string> | null = null;

export function getOrCreateWelcomeSession(): Promise<string> {
  if (!pendingWelcomeSession) {
    pendingWelcomeSession = findOrCreateWelcomeSession().finally(() => {
      pendingWelcomeSession = null;
    });
  }
  return pendingWelcomeSession;
}

export function setPendingWelcomeSession(sessionId: string | null) {
  if (sessionId) {
    localStorage.setItem(PENDING_WELCOME_SESSION_KEY, sessionId);
  } else {
    localStorage.removeItem(PENDING_WELCOME_SESSION_KEY);
  }
}

export function takePendingWelcomeSession(): string | null {
  const sessionId = localStorage.getItem(PENDING_WELCOME_SESSION_KEY);
  localStorage.removeItem(PENDING_WELCOME_SESSION_KEY);
  return sessionId;
}

async function findOrCreateWelcomeSession(): Promise<string> {
  const rows = await liveQueryClient.execute<{ id: string }>(
    `
      SELECT id
      FROM sessions
      WHERE deleted_at IS NULL
        AND CASE
          WHEN json_valid(event_json)
          THEN json_extract(event_json, '$.tracking_id')
        END = ?
      ORDER BY created_at, id
      LIMIT 1
    `,
    [TRACKING_ID],
  );
  if (rows[0]) return rows[0].id;

  const now = new Date().toISOString();
  const event: SessionEvent = {
    tracking_id: TRACKING_ID,
    calendar_id: "",
    title: "Welcome to Anarlog",
    started_at: now,
    ended_at: "",
    is_all_day: false,
    has_recurrence_rules: false,
    meeting_link: DEMO_URL,
    description: "A private, prerecorded introduction to Anarlog.",
  };

  return createSession("Welcome to Anarlog", DEFAULT_USER_ID, {
    event_json: JSON.stringify(event),
    raw_md: JSON.stringify(md2json(WELCOME_NOTE)),
  });
}
