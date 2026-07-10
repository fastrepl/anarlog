import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { SessionContentData } from "@hypr/plugin-fs-sync";
import type { SessionContext, Transcript } from "@hypr/plugin-template";

import { loadHumansByIds } from "~/contacts/queries";
import { loadSessionParticipantHumanIds } from "~/session/queries";
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
  transcriptData: SessionContentData["transcript"],
  humans: Array<{ id: string; name: string }>,
  participantHumanIds: string[],
  selfHumanId?: string,
): Promise<Transcript | null> {
  const transcripts = transcriptData?.transcripts ?? [];
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

  const startedAtCandidates = transcripts
    .map((t) => t.started_at)
    .filter((v): v is number => typeof v === "number");
  const endedAtCandidates = transcripts
    .map((t) => t.ended_at)
    .filter((v): v is number => typeof v === "number");

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

export async function hydrateSessionContextFromFs(
  sessionId: string,
  selfHumanId?: string,
): Promise<SessionContext | null> {
  const result = await fsSyncCommands.loadSessionContent(sessionId);
  if (result.status === "error") {
    return null;
  }

  const payload = result.data;
  const sqliteParticipantHumanIds =
    await loadSessionParticipantHumanIds(sessionId);
  const legacyParticipantHumanIds =
    payload.meta?.participants?.map((participant) => participant.humanId) ?? [];
  const participantHumanIds = [
    ...new Set(
      [...sqliteParticipantHumanIds, ...legacyParticipantHumanIds].filter(
        Boolean,
      ),
    ),
  ];
  const assignedHumanIds = collectAssignedHumanIdsFromTranscriptRows(
    payload.transcript?.transcripts ?? [],
  );
  const humanIds = [
    ...new Set(
      [...participantHumanIds, ...assignedHumanIds, selfHumanId ?? ""].filter(
        Boolean,
      ),
    ),
  ];
  const humans = await loadHumansByIds(humanIds);
  const humansById = new Map(humans.map((human) => [human.id, human]));
  const participants = participantHumanIds.flatMap((humanId) => {
    const human = humansById.get(humanId);
    return human?.name
      ? [{ name: human.name, jobTitle: human.jobTitle || null }]
      : [];
  });

  const enhancedContent = payload.notes
    .slice()
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0))
    .map((note) => note.markdown ?? null)
    .filter((note): note is string => Boolean(note))
    .join("\n\n---\n\n");

  const transcript = await buildTranscript(
    payload.transcript,
    humans,
    participantHumanIds,
    selfHumanId,
  );
  const eventName = extractEventName(payload.meta?.event);

  return {
    title: payload.meta?.title ?? null,
    date: payload.meta?.createdAt ?? null,
    rawContent: payload.rawMemoMarkdown ?? null,
    enhancedContent: enhancedContent || null,
    transcript,
    participants,
    event: eventName ? { name: eventName } : null,
  };
}
