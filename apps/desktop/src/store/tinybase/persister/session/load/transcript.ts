import type { TranscriptJson, TranscriptWithData } from "@hypr/plugin-fs-sync";

import type { LoadedSessionData } from "./types";

const LABEL = "SessionPersister";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function asRoundedNumber(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.round(value)
    : 0;
}

function asOptionalRoundedNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.round(value)
    : undefined;
}

function normalizeTranscriptWords(value: unknown): TranscriptWithData["words"] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.map((word) => normalizeTranscriptWord(word));
}

function normalizeTranscriptWord(
  value: unknown,
): NonNullable<TranscriptWithData["words"]>[number] {
  if (!isRecord(value)) {
    return value as NonNullable<TranscriptWithData["words"]>[number];
  }

  const startMs = asOptionalRoundedNumber(value.start_ms);
  const endMs = asOptionalRoundedNumber(value.end_ms);

  return {
    ...(value as NonNullable<TranscriptWithData["words"]>[number]),
    ...(startMs === undefined ? {} : { start_ms: startMs }),
    ...(endMs === undefined ? {} : { end_ms: endMs }),
  };
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
      started_at: asRoundedNumber(value.started_at),
      ended_at: asOptionalRoundedNumber(value.ended_at),
      memo_md: asString(value.memo_md),
      words: normalizeTranscriptWords(value.words),
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
): boolean {
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

    return true;
  } catch (error) {
    console.error(`[${LABEL}] Failed to load transcript from ${path}:`, error);
    return false;
  }
}
