import type { SessionContext, Transcript } from "@anlg/plugin-template";
import { commands as transcriptionCommands } from "@anlg/plugin-transcription";

import { loadSessionContentSnapshot } from "~/session/content-queries";
import {
  formatMeetingChatRecordsAsMarkdown,
  loadMeetingChatRecords,
} from "~/stt/meeting-chat-records";

function extractEventName(event: unknown): string | null {
  if (!event || typeof event !== "object") {
    return null;
  }

  const record = event as Record<string, unknown>;
  if (typeof record.name === "string" && record.name) {
    return record.name;
  }
  if (typeof record.title === "string" && record.title) {
    return record.title;
  }

  return null;
}

async function buildTranscript(
  sessionId: string,
  selfHumanId?: string,
): Promise<Transcript | null> {
  const result = await transcriptionCommands.renderSessionTranscript({
    session_id: sessionId,
    self_human_id: selfHumanId ?? null,
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }
  if (!result.data) {
    return null;
  }

  return {
    segments: result.data.segments.map((segment) => ({
      speaker: segment.speaker_label,
      text: segment.text,
      startMs: segment.start_ms,
    })),
    startedAt: result.data.started_at,
    endedAt: result.data.ended_at,
  };
}

export async function hydrateSessionContext(
  sessionId: string,
  selfHumanId?: string,
): Promise<SessionContext | null> {
  const snapshot = await loadSessionContentSnapshot(sessionId, {
    includeTranscriptWords: false,
  });
  if (!snapshot) return null;

  const participants = snapshot.participants.flatMap((participant) =>
    participant.name
      ? [{ name: participant.name, jobTitle: participant.jobTitle || null }]
      : [],
  );

  const enhancedContent = snapshot.enhancedNotes
    .map((note) => note.markdown || null)
    .filter((note): note is string => Boolean(note))
    .join("\n\n---\n\n");

  const transcript = await buildTranscript(sessionId, selfHumanId);
  const eventName = extractEventName(snapshot.event);
  const meetingChat = formatMeetingChatRecordsAsMarkdown(
    await loadMeetingChatRecords(sessionId),
  );

  return {
    sessionId,
    title: snapshot.title || null,
    date: snapshot.createdAt || null,
    rawContent: snapshot.rawMarkdown || null,
    enhancedContent: enhancedContent || null,
    meetingChat: meetingChat || null,
    transcript,
    participants,
    event: eventName ? { name: eventName } : null,
  };
}
