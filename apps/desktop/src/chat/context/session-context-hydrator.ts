import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { SessionContentData } from "@hypr/plugin-fs-sync";
import type { SessionContext, Transcript } from "@hypr/plugin-template";

import type * as main from "~/store/tinybase/store/main";
import {
  buildRenderTranscriptRequestFromFsTranscript,
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
  store: ReturnType<typeof main.UI.useStore>,
  sessionId: string,
): Promise<Transcript | null> {
  const transcripts = transcriptData?.transcripts ?? [];
  if (transcripts.length === 0) {
    return null;
  }
  const request = buildRenderTranscriptRequestFromFsTranscript(
    transcriptData,
    store,
    sessionId,
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
  store: ReturnType<typeof main.UI.useStore>,
  sessionId: string,
): Promise<SessionContext | null> {
  const result = await fsSyncCommands.loadSessionContent(sessionId);
  if (result.status === "error") {
    return null;
  }

  const payload = result.data;
  const participants =
    payload.meta?.participants
      ?.map((participant) => {
        const row = store?.getRow("humans", participant.humanId);
        if (!row || typeof row.name !== "string" || !row.name) {
          return null;
        }

        return {
          name: row.name,
          jobTitle:
            typeof row.job_title === "string" && row.job_title
              ? row.job_title
              : null,
        };
      })
      .filter(
        (
          participant,
        ): participant is { name: string; jobTitle: string | null } =>
          Boolean(participant),
      ) ?? [];

  const enhancedContent = payload.notes
    .slice()
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0))
    .map((note) => note.markdown ?? null)
    .filter((note): note is string => Boolean(note))
    .join("\n\n---\n\n");

  const transcript = await buildTranscript(
    payload.transcript,
    store,
    sessionId,
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
