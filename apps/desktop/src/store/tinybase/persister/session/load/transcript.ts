import type { TranscriptJson, TranscriptWithData } from "@hypr/plugin-fs-sync";

import type { LoadedSessionData } from "./types";

const LABEL = "SessionPersister";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function asNumber(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function asOptionalNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value)
    ? value
    : undefined;
}

function normalizeTranscript(value: unknown): TranscriptWithData[] {
  if (!isRecord(value)) {
    return [];
  }

  const id = typeof value.id === "string" ? value.id : "";
  const session_id =
    typeof value.session_id === "string" ? value.session_id : "";
  if (!id || !session_id) {
    return [];
  }

  return [
    {
      id,
      user_id: asString(value.user_id),
      created_at: asString(value.created_at),
      session_id,
      started_at: asNumber(value.started_at),
      ended_at: asOptionalNumber(value.ended_at),
      memo_md: asString(value.memo_md),
      words: Array.isArray(value.words)
        ? (value.words as TranscriptWithData["words"])
        : [],
      speaker_hints: Array.isArray(value.speaker_hints)
        ? (value.speaker_hints as TranscriptWithData["speaker_hints"])
        : [],
    },
  ];
}

function parseTranscriptJson(content: string): TranscriptJson {
  const value = JSON.parse(content) as unknown;
  if (!isRecord(value) || !Array.isArray(value.transcripts)) {
    return { transcripts: [] };
  }

  return {
    transcripts: value.transcripts.flatMap((transcript) =>
      normalizeTranscript(transcript),
    ),
  };
}

export function processTranscriptFile(
  path: string,
  content: string,
  result: LoadedSessionData,
): void {
  try {
    const data = parseTranscriptJson(content);

    for (const transcript of data.transcripts ?? []) {
      const { id, words, speaker_hints, ...transcriptData } = transcript;
      result.transcripts[id] = {
        ...transcriptData,
        user_id: transcriptData.user_id ?? "",
        created_at: transcriptData.created_at ?? "",
        started_at:
          typeof transcriptData.started_at === "number"
            ? transcriptData.started_at
            : 0,
        ended_at:
          typeof transcriptData.ended_at === "number"
            ? transcriptData.ended_at
            : undefined,
        memo_md:
          typeof transcriptData.memo_md === "string"
            ? transcriptData.memo_md
            : "",
        words: JSON.stringify(words ?? []),
        speaker_hints: JSON.stringify(speaker_hints ?? []),
      };
    }
  } catch (error) {
    console.error(`[${LABEL}] Failed to load transcript from ${path}:`, error);
  }
}
