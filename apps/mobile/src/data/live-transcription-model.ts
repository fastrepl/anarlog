import { assertBoundedTranscriptionResponse } from "./transcription-response.ts";

const MAX_STREAM_MESSAGE_BYTES = 1_000_000;
const MAX_WORDS_PER_MESSAGE = 5_000;
const SYNTHETIC_WORD_MS = 400;

type StreamWord = {
  word?: unknown;
  punctuated_word?: unknown;
  start?: unknown;
  end?: unknown;
  speaker?: unknown;
};

export type LiveTranscriptWord = {
  id: string;
  text: string;
  start_ms: number;
  end_ms: number;
  channel: number;
  state: "final";
  speaker_index?: number;
};

export type LiveSpeakerHint = {
  id: string;
  word_id: string;
  type: "provider_speaker_index";
  value: string;
};

export type HostedTranscriptEvent =
  | { type: "partial"; text: string }
  | {
      type: "final";
      segmentId: string;
      text: string;
      words: LiveTranscriptWord[];
      hints: LiveSpeakerHint[];
    }
  | { type: "terminal" }
  | { type: "error"; message: string };

function finiteNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function normalizedChannel(value: unknown): number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0
    ? value
    : 0;
}

function mapFinalWords({
  transcriptId,
  provider,
  segmentId,
  channel,
  streamWords,
  transcript,
  start,
  duration,
}: {
  transcriptId: string;
  provider: string;
  segmentId: string;
  channel: number;
  streamWords: StreamWord[];
  transcript: string;
  start: number;
  duration: number;
}): { words: LiveTranscriptWord[]; hints: LiveSpeakerHint[] } {
  const sourceWords = streamWords.slice(0, MAX_WORDS_PER_MESSAGE);
  const words: LiveTranscriptWord[] = [];
  const hints: LiveSpeakerHint[] = [];

  sourceWords.forEach((word, index) => {
    const rawText =
      typeof word.punctuated_word === "string" && word.punctuated_word !== ""
        ? word.punctuated_word
        : word.word;
    if (typeof rawText !== "string" || rawText.trim() === "") return;
    const startMs = Math.max(
      0,
      Math.round(finiteNumber(word.start, start) * 1_000),
    );
    const endMs = Math.max(
      startMs,
      Math.round(finiteNumber(word.end, start) * 1_000),
    );
    const wordId = `${transcriptId}:${segmentId}:${index}`;
    const speaker =
      typeof word.speaker === "number" && Number.isInteger(word.speaker)
        ? word.speaker
        : undefined;
    words.push({
      id: wordId,
      text: rawText,
      start_ms: startMs,
      end_ms: endMs,
      channel,
      state: "final",
      ...(speaker === undefined ? {} : { speaker_index: speaker }),
    });
    if (speaker !== undefined) {
      hints.push({
        id: `${wordId}:speaker`,
        word_id: wordId,
        type: "provider_speaker_index",
        value: JSON.stringify({
          provider,
          channel,
          speaker_index: speaker,
        }),
      });
    }
  });

  if (words.length > 0 || transcript.trim() === "") return { words, hints };
  const tokens = transcript.trim().split(/\s+/).slice(0, MAX_WORDS_PER_MESSAGE);
  const durationMs = Math.max(
    Math.round(Math.max(0, duration) * 1_000),
    tokens.length * SYNTHETIC_WORD_MS,
  );
  const startMs = Math.max(0, Math.round(start * 1_000));
  return {
    words: tokens.map((text, index) => ({
      id: `${transcriptId}:${segmentId}:${index}`,
      text,
      start_ms: startMs + Math.round((index / tokens.length) * durationMs),
      end_ms: startMs + Math.round(((index + 1) / tokens.length) * durationMs),
      channel,
      state: "final",
    })),
    hints,
  };
}

function parsePayload(
  payload: unknown,
  transcriptId: string,
  provider: string,
): HostedTranscriptEvent[] {
  if (Array.isArray(payload)) {
    return payload.flatMap((item) =>
      parsePayload(item, transcriptId, provider),
    );
  }
  if (!payload || typeof payload !== "object") return [];
  const response = payload as Record<string, unknown>;
  if (response.type === "Metadata") return [{ type: "terminal" }];
  if (response.type === "Error") {
    return [
      {
        type: "error",
        message:
          typeof response.error_message === "string"
            ? response.error_message
            : "Live transcription failed",
      },
    ];
  }
  if (response.type !== "Results") return [];

  const channelObject =
    response.channel && typeof response.channel === "object"
      ? (response.channel as { alternatives?: unknown })
      : null;
  const alternatives = Array.isArray(channelObject?.alternatives)
    ? channelObject.alternatives
    : [];
  const alternative =
    alternatives[0] && typeof alternatives[0] === "object"
      ? (alternatives[0] as { transcript?: unknown; words?: unknown })
      : null;
  const transcript =
    typeof alternative?.transcript === "string" ? alternative.transcript : "";
  const streamWords = Array.isArray(alternative?.words)
    ? (alternative.words as StreamWord[])
    : [];
  const channelIndex = Array.isArray(response.channel_index)
    ? response.channel_index[0]
    : 0;
  const channel = normalizedChannel(channelIndex);
  const start = Math.max(0, finiteNumber(response.start, 0));
  const duration = Math.max(0, finiteNumber(response.duration, 0));
  if (response.is_final !== true) {
    return transcript.trim() === ""
      ? []
      : [{ type: "partial", text: transcript.trim() }];
  }

  const segmentId = `${channel}:${Math.round(start * 1_000)}`;
  const { words, hints } = mapFinalWords({
    transcriptId,
    provider,
    segmentId,
    channel,
    streamWords,
    transcript,
    start,
    duration,
  });
  return words.length === 0
    ? []
    : [{ type: "final", segmentId, text: transcript.trim(), words, hints }];
}

export function parseHostedTranscriptionMessage(
  message: string,
  transcriptId: string,
  provider = "anarlog",
): HostedTranscriptEvent[] {
  try {
    assertBoundedTranscriptionResponse(message, null, MAX_STREAM_MESSAGE_BYTES);
  } catch {
    return [
      { type: "error", message: "Live transcription response is too large" },
    ];
  }
  try {
    return parsePayload(JSON.parse(message), transcriptId, provider);
  } catch {
    return [
      { type: "error", message: "Live transcription returned invalid data" },
    ];
  }
}
