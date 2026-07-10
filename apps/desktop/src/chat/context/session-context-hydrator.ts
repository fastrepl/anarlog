import type { SessionContext, Transcript } from "@hypr/plugin-template";

import { loadHumansByIds } from "~/contacts/queries";
import {
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import {
  buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

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
  transcripts: SessionContentSnapshot["transcripts"],
  humans: Array<{ id: string; name: string }>,
  participantHumanIds: string[],
  selfHumanId?: string,
): Promise<Transcript | null> {
  if (transcripts.length === 0) {
    return null;
  }
  const request = buildRenderTranscriptRequestFromRows(
    transcripts,
    {
      selfHumanId,
      humans: humans
        .filter((human) => human.name)
        .map((human) => ({ human_id: human.id, name: human.name })),
    },
    participantHumanIds,
  );
  if (!request) {
    return null;
  }
  const segments = await renderTranscriptSegments(request);

  const startedAtCandidates = transcripts.map(
    (transcript) => transcript.started_at,
  );
  const endedAtCandidates = transcripts
    .map((transcript) => transcript.ended_at)
    .filter((value): value is number => typeof value === "number");

  return {
    segments: segments.map((segment) => ({
      speaker: segment.speaker_label,
      text: segment.text,
    })),
    startedAt:
      startedAtCandidates.length > 0 ? Math.min(...startedAtCandidates) : null,
    endedAt:
      endedAtCandidates.length > 0 ? Math.max(...endedAtCandidates) : null,
  };
}

export async function hydrateSessionContext(
  sessionId: string,
  selfHumanId?: string,
): Promise<SessionContext | null> {
  const snapshot = await loadSessionContentSnapshot(sessionId);
  if (!snapshot) return null;

  const participantHumanIds = snapshot.participants.map(
    (participant) => participant.humanId,
  );
  const assignedHumanIds = collectAssignedHumanIdsFromTranscriptRows(
    snapshot.transcripts,
  );
  const humanIds = [
    ...new Set(
      [...participantHumanIds, ...assignedHumanIds, selfHumanId ?? ""].filter(
        Boolean,
      ),
    ),
  ];
  const humans = await loadHumansByIds(humanIds);
  const participants = snapshot.participants.flatMap((participant) =>
    participant.name
      ? [{ name: participant.name, jobTitle: participant.jobTitle || null }]
      : [],
  );

  const enhancedContent = snapshot.enhancedNotes
    .map((note) => note.markdown || null)
    .filter((note): note is string => Boolean(note))
    .join("\n\n---\n\n");

  const transcript = await buildTranscript(
    snapshot.transcripts,
    humans,
    participantHumanIds,
    selfHumanId,
  );
  const eventName = extractEventName(snapshot.event);

  return {
    title: snapshot.title || null,
    date: snapshot.createdAt || null,
    rawContent: snapshot.rawMarkdown || null,
    enhancedContent: enhancedContent || null,
    transcript,
    participants,
    event: eventName ? { name: eventName } : null,
  };
}
