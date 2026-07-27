import { md2json } from "@hypr/editor/markdown";
import type { SessionEvent } from "@hypr/store";

import { liveQueryClient } from "~/db";
import {
  WELCOME_NOTE_DEMO_URL,
  WELCOME_NOTE_TRACKING_ID,
} from "~/onboarding/welcome-note.constants";
import { createSession } from "~/session/queries";
import { DEFAULT_USER_ID } from "~/shared/utils";
import { listenerStore } from "~/store/zustand/listener/instance";

const PENDING_WELCOME_SESSION_KEY = "anarlog.pending-welcome-session";

const WELCOME_NOTE = `Welcome to Anarlog 👋


This note is a quick way to see how Anarlog works.


Click **Join & record** in the top-right corner. It will open a private, prerecorded demo meeting, so you don't have to worry about your camera or microphone. Anarlog will save the audio. To create a transcript and notes, choose a provider in **Settings → Transcription**; if one is not ready, Anarlog will show you a setup shortcut.


When the video ends, Anarlog will stop listening. If transcription and intelligence are configured, it will start creating your summary automatically.`;

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

export async function stopActiveWelcomeDemo() {
  const active = listenerStore.getState().live;
  const sessionId = active.sessionId;
  if (!sessionId || active.status !== "active") {
    return;
  }
  const captureGeneration = active.captureGenerationBySession[sessionId];

  const rows = await liveQueryClient.execute<{ id: string }>(
    `
      SELECT id
      FROM sessions
      WHERE id = ?
        AND deleted_at IS NULL
        AND CASE
          WHEN json_valid(event_json)
          THEN json_extract(event_json, '$.tracking_id')
        END = ?
      LIMIT 1
    `,
    [sessionId, WELCOME_NOTE_TRACKING_ID],
  );
  if (!rows[0]) {
    return;
  }

  const current = listenerStore.getState();
  if (
    current.live.sessionId === sessionId &&
    current.live.status === "active" &&
    current.live.captureGenerationBySession[sessionId] === captureGeneration
  ) {
    current.stop();
  }
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
    [WELCOME_NOTE_TRACKING_ID],
  );
  if (rows[0]) return rows[0].id;

  const now = new Date().toISOString();
  const event: SessionEvent = {
    tracking_id: WELCOME_NOTE_TRACKING_ID,
    calendar_id: "",
    title: "Welcome to Anarlog",
    started_at: now,
    ended_at: "",
    is_all_day: false,
    has_recurrence_rules: false,
    meeting_link: WELCOME_NOTE_DEMO_URL,
    description: "A private, prerecorded introduction to Anarlog.",
  };

  return createSession("Welcome to Anarlog", DEFAULT_USER_ID, {
    event_json: JSON.stringify(event),
    raw_md: JSON.stringify(md2json(WELCOME_NOTE)),
  });
}
