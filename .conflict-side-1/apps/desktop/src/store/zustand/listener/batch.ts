import type { StoreApi } from "zustand";

import type {
  BatchErrorCode,
  BatchResponse,
  BatchStreamEvent,
} from "@hypr/plugin-transcription";

import type { BatchPersistCallback } from "./transcript";
import { transformWordEntries, type WordEntry } from "./utils";

import { type RuntimeSpeakerHint, type WordLike } from "~/stt/segment";
import {
  createTranscriptTimingMetadata,
  getValidTimingSource,
  type TranscriptTimingSource,
} from "~/stt/timing";

export type BatchPhase = "importing" | "transcribing";
export type BatchTerminalReason = "failed" | "timed_out" | "stopped";

export type BatchState = {
  batch: Record<
    string,
    {
      percentage: number;
      isComplete?: boolean;
      error?: string;
      phase?: BatchPhase;
      terminalReason?: BatchTerminalReason;
      errorCode?: BatchErrorCode;
    }
  >;
  batchPreview: Record<
    string,
    {
      wordsByChannel: Record<number, WordLike[]>;
      hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
    }
  >;
  batchPersist: Record<string, BatchPersistCallback>;
};

export type BatchActions = {
  handleBatchStarted: (sessionId: string, phase?: BatchPhase) => void;
  handleBatchCompleted: (sessionId: string) => void;
  handleBatchResponse: (sessionId: string, response: BatchResponse) => boolean;
  handleBatchResponseStreamed: (
    sessionId: string,
    event: BatchStreamEvent,
  ) => void;
  handleBatchFailed: (
    sessionId: string,
    error: string,
    terminalReason?: Exclude<BatchTerminalReason, "stopped">,
    errorCode?: BatchErrorCode,
  ) => void;
  handleBatchStopped: (sessionId: string) => void;
  updateBatchProgress: (sessionId: string, percentage: number) => void;
  clearBatchSession: (sessionId: string) => void;
  setBatchPersist: (sessionId: string, callback: BatchPersistCallback) => void;
  clearBatchPersist: (sessionId: string) => void;
};

export const EMPTY_BATCH_TRANSCRIPT_ERROR =
  "No speech was detected in the audio.";
const SYNTHETIC_TEXT_WORD_SECONDS = 0.4;
const MIN_SYNTHETIC_TEXT_WORD_SECONDS = 0.05;

export const createBatchSlice = <T extends BatchState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): BatchState & BatchActions => ({
  batch: {},
  batchPreview: {},
  batchPersist: {},

  handleBatchStarted: (sessionId, phase) => {
    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          percentage: 0,
          isComplete: false,
          phase: phase ?? "transcribing",
          terminalReason: undefined,
          error: undefined,
          errorCode: undefined,
        },
      },
      batchPreview: {
        ...state.batchPreview,
        [sessionId]: {
          wordsByChannel: {},
          hintsByChannel: {},
        },
      },
    }));
  },

  handleBatchCompleted: (sessionId) => {
    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          ...(state.batch[sessionId] ?? { percentage: 1 }),
          percentage: 1,
          isComplete: true,
          phase: "transcribing",
          terminalReason: undefined,
          error: undefined,
          errorCode: undefined,
        },
      },
    }));
  },

  handleBatchResponse: (sessionId, response) => {
    const persist = get().batchPersist[sessionId];

    const [words, hints] = transformBatch(response);
    if (!words.length) {
      return false;
    }

    persist?.(words, hints, { mode: "replace" });

    set((state) => {
      if (!state.batch[sessionId]) {
        return state;
      }

      const { [sessionId]: _, ...rest } = state.batch;
      const { [sessionId]: __, ...restPreview } = state.batchPreview;
      return {
        ...state,
        batch: rest,
        batchPreview: restPreview,
      };
    });

    return true;
  },

  handleBatchResponseStreamed: (sessionId, event) => {
    const percentage = getBatchStreamPercentage(event);
    const isComplete = event.type === "result" || event.type === "terminal";
    const currentPreview = get().batchPreview[sessionId] ?? {
      wordsByChannel: {},
      hintsByChannel: {},
    };
    const persist = get().batchPersist[sessionId];
    const result = batchPreviewResult(currentPreview, event);
    const { nextPreview, words, hints } = result;

    if (result.shouldPersist && words.length > 0) {
      persist?.(words, hints, { mode: "replace" });
    }

    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          percentage,
          isComplete: isComplete || false,
          phase: "transcribing",
          terminalReason: undefined,
          error: undefined,
          errorCode: undefined,
        },
      },
      batchPreview: {
        ...state.batchPreview,
        [sessionId]: nextPreview,
      },
    }));
  },

  updateBatchProgress: (sessionId, percentage) => {
    set((state) => {
      const entry = state.batch[sessionId];
      if (!entry) {
        return state;
      }
      return {
        ...state,
        batch: {
          ...state.batch,
          [sessionId]: { ...entry, percentage },
        },
      };
    });
  },

  handleBatchFailed: (
    sessionId,
    error,
    terminalReason = "failed",
    errorCode,
  ) => {
    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          ...(state.batch[sessionId] ?? { percentage: 0 }),
          error,
          isComplete: false,
          terminalReason,
          errorCode,
        },
      },
      batchPreview: {
        ...state.batchPreview,
        [sessionId]: {
          wordsByChannel: {},
          hintsByChannel: {},
        },
      },
    }));
  },

  handleBatchStopped: (sessionId) => {
    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          ...(state.batch[sessionId] ?? { percentage: 0 }),
          error: "Transcription stopped.",
          isComplete: false,
          terminalReason: "stopped",
          errorCode: undefined,
        },
      },
      batchPreview: {
        ...state.batchPreview,
        [sessionId]: {
          wordsByChannel: {},
          hintsByChannel: {},
        },
      },
    }));
  },

  clearBatchSession: (sessionId) => {
    set((state) => {
      if (!(sessionId in state.batch)) {
        return state;
      }

      const { [sessionId]: _, ...rest } = state.batch;
      const { [sessionId]: __, ...restPreview } = state.batchPreview;
      return {
        ...state,
        batch: rest,
        batchPreview: restPreview,
      };
    });
  },

  setBatchPersist: (sessionId, callback) => {
    set((state) => ({
      ...state,
      batchPersist: {
        ...state.batchPersist,
        [sessionId]: callback,
      },
    }));
  },

  clearBatchPersist: (sessionId) => {
    set((state) => {
      if (!(sessionId in state.batchPersist)) {
        return state;
      }

      const { [sessionId]: _, ...rest } = state.batchPersist;
      return {
        ...state,
        batchPersist: rest,
      };
    });
  },
});

function batchPreviewResult(
  currentPreview: {
    wordsByChannel: Record<number, WordLike[]>;
    hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
  },
  event: BatchStreamEvent,
): {
  nextPreview: {
    wordsByChannel: Record<number, WordLike[]>;
    hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
  };
  words: WordLike[];
  hints: RuntimeSpeakerHint[];
  shouldPersist: boolean;
} {
  if (event.type === "result") {
    const [words, hints] = transformBatch(event.response);
    return {
      nextPreview: batchPreviewFromWords(words, hints),
      words,
      hints,
      shouldPersist: true,
    };
  }

  const nextPreview = mergeBatchPreview(currentPreview, event);
  const [words, hints] = flattenBatchPreview(nextPreview);

  return {
    nextPreview,
    words,
    hints,
    shouldPersist: event.type === "segment",
  };
}

function transformBatch(
  response: BatchResponse,
): [WordLike[], RuntimeSpeakerHint[]] {
  const allWords: WordLike[] = [];
  const allHints: RuntimeSpeakerHint[] = [];
  let wordOffset = 0;

  response.results.channels.forEach((channel, channelIndex) => {
    const alternative = channel.alternatives[0];
    if (!alternative) {
      return;
    }

    const timingSource = getWordTimingSourceForBatchResponse(
      response,
      Boolean(alternative.words?.length),
      "synthetic_text",
    );
    const wordEntries = wordEntriesFromTranscript(
      alternative.words,
      alternative.transcript,
      {
        channel: channelIndex,
        durationSeconds: getBatchDurationSeconds(response),
        timingSource,
      },
    );

    const [words, hints] = transformWordEntries(
      wordEntries,
      alternative.transcript,
      channelIndex,
      { timingSource },
    );

    hints.forEach((hint) => {
      allHints.push({
        ...hint,
        wordIndex: hint.wordIndex + wordOffset,
      });
    });
    allWords.push(...words);
    wordOffset += words.length;
  });

  return [allWords, allHints];
}

function flattenBatchPreview(preview: {
  wordsByChannel: Record<number, WordLike[]>;
  hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
}): [WordLike[], RuntimeSpeakerHint[]] {
  const allWords: WordLike[] = [];
  const allHints: RuntimeSpeakerHint[] = [];
  let wordOffset = 0;

  Object.keys(preview.wordsByChannel)
    .map(Number)
    .sort((a, b) => a - b)
    .forEach((channel) => {
      const words = preview.wordsByChannel[channel] ?? [];
      const hints = preview.hintsByChannel[channel] ?? [];

      hints.forEach((hint) => {
        allHints.push({
          ...hint,
          wordIndex: hint.wordIndex + wordOffset,
        });
      });
      allWords.push(...words);
      wordOffset += words.length;
    });

  return [allWords, allHints];
}

function batchPreviewFromWords(
  words: WordLike[],
  hints: RuntimeSpeakerHint[],
): {
  wordsByChannel: Record<number, WordLike[]>;
  hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
} {
  const wordsByChannel: Record<number, WordLike[]> = {};
  const hintsByChannel: Record<number, RuntimeSpeakerHint[]> = {};
  const channelIndexByWordIndex = new Map<
    number,
    { channel: number; wordIndex: number }
  >();

  words.forEach((word, wordIndex) => {
    const channelWords = wordsByChannel[word.channel] ?? [];
    if (!(word.channel in wordsByChannel)) {
      wordsByChannel[word.channel] = channelWords;
      hintsByChannel[word.channel] = [];
    }

    channelIndexByWordIndex.set(wordIndex, {
      channel: word.channel,
      wordIndex: channelWords.length,
    });
    channelWords.push(word);
  });

  hints.forEach((hint) => {
    const target = channelIndexByWordIndex.get(hint.wordIndex);
    if (!target) {
      return;
    }

    hintsByChannel[target.channel]!.push({
      ...hint,
      wordIndex: target.wordIndex,
    });
  });

  return { wordsByChannel, hintsByChannel };
}

function mergeBatchPreview(
  preview: {
    wordsByChannel: Record<number, WordLike[]>;
    hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
  },
  event: BatchStreamEvent,
) {
  if (event.type !== "segment") {
    return preview;
  }

  const response = event.response;
  if (response.type !== "Results") {
    return preview;
  }

  const channelIndex = response.channel_index[0];
  const alternative = response.channel.alternatives[0];
  if (channelIndex === undefined || !alternative) {
    return preview;
  }

  const timingSource = getWordTimingSourceForBatchResponse(
    response,
    Boolean(alternative.words?.length),
    "provider_segment_interpolated",
  );
  const wordEntries = wordEntriesFromTranscript(
    alternative.words,
    alternative.transcript,
    {
      channel: channelIndex,
      startSeconds: response.start,
      durationSeconds: response.duration,
      timingSource,
    },
  );

  const [incomingWords, incomingHints] = transformWordEntries(
    wordEntries,
    alternative.transcript,
    channelIndex,
    { timingSource },
  );
  if (incomingWords.length === 0) {
    return preview;
  }

  if (response.from_finalize) {
    return {
      wordsByChannel: {
        ...preview.wordsByChannel,
        [channelIndex]: incomingWords,
      },
      hintsByChannel: {
        ...preview.hintsByChannel,
        [channelIndex]: incomingHints,
      },
    };
  }

  const existingWords = preview.wordsByChannel[channelIndex] ?? [];
  const existingHints = preview.hintsByChannel[channelIndex] ?? [];
  const firstStartMs = incomingWords[0]?.start_ms ?? 0;
  const lastEndMs = incomingWords[incomingWords.length - 1]?.end_ms ?? 0;

  const beforeWords = existingWords.filter(
    (word) => word.end_ms <= firstStartMs,
  );
  const afterWords = existingWords.filter((word) => word.start_ms >= lastEndMs);
  const mergedWords = [...beforeWords, ...incomingWords, ...afterWords];

  const hintsBefore = existingHints.filter((hint) => {
    const word = existingWords[hint.wordIndex];
    return word && word.end_ms <= firstStartMs;
  });
  const afterIndexMap = new Map<number, number>();
  let afterIndex = 0;
  for (let index = 0; index < existingWords.length; index += 1) {
    if (existingWords[index].start_ms >= lastEndMs) {
      afterIndexMap.set(
        index,
        beforeWords.length + incomingWords.length + afterIndex,
      );
      afterIndex += 1;
    }
  }
  const hintsAfter = existingHints
    .filter((hint) => afterIndexMap.has(hint.wordIndex))
    .map((hint) => ({
      ...hint,
      wordIndex: afterIndexMap.get(hint.wordIndex)!,
    }));
  const adjustedIncomingHints = incomingHints.map((hint) => ({
    ...hint,
    wordIndex: beforeWords.length + hint.wordIndex,
  }));

  return {
    wordsByChannel: {
      ...preview.wordsByChannel,
      [channelIndex]: mergedWords,
    },
    hintsByChannel: {
      ...preview.hintsByChannel,
      [channelIndex]: [...hintsBefore, ...adjustedIncomingHints, ...hintsAfter],
    },
  };
}

function getBatchStreamPercentage(event: BatchStreamEvent): number {
  switch (event.type) {
    case "progress":
    case "segment":
      return event.percentage;
    case "result":
    case "terminal":
      return 1;
    case "error":
      return 0;
  }
}

function wordEntriesFromTranscript(
  entries: WordEntry[] | null | undefined,
  transcript: string,
  {
    channel,
    startSeconds = 0,
    durationSeconds,
    timingSource,
  }: {
    channel: number;
    startSeconds?: number;
    durationSeconds?: number;
    timingSource: TranscriptTimingSource;
  },
): WordEntry[] {
  if (entries?.length) {
    return entries.map((entry) => ({
      ...entry,
      metadata: createTranscriptTimingMetadata(timingSource, entry.metadata),
    }));
  }

  if (!transcript.trim()) {
    return [];
  }

  const tokens = transcript.trim().split(/\s+/).filter(Boolean);
  if (!tokens.length) {
    return [];
  }

  const duration =
    timingSource === "synthetic_text"
      ? tokens.length * SYNTHETIC_TEXT_WORD_SECONDS
      : Math.max(
          durationSeconds && Number.isFinite(durationSeconds)
            ? durationSeconds
            : tokens.length * SYNTHETIC_TEXT_WORD_SECONDS,
          tokens.length * MIN_SYNTHETIC_TEXT_WORD_SECONDS,
        );

  return tokens.map((token, index) => ({
    word: token,
    punctuated_word: token,
    start: startSeconds + (index / tokens.length) * duration,
    end: startSeconds + ((index + 1) / tokens.length) * duration,
    channel,
    speaker: null,
    metadata: createTranscriptTimingMetadata(timingSource),
  }));
}

function getWordTimingSourceForBatchResponse(
  response: { metadata?: unknown },
  hasProviderWords: boolean,
  fallbackWithoutWords: TranscriptTimingSource,
): TranscriptTimingSource {
  if (!hasProviderWords) {
    return fallbackWithoutWords;
  }

  const explicitSource = getBatchResponseTimingSource(response);
  if (explicitSource) {
    return explicitSource;
  }

  return "provider_word";
}

function getBatchResponseTimingSource(response: {
  metadata?: unknown;
}): TranscriptTimingSource | undefined {
  const metadata = response.metadata;
  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata)) {
    return undefined;
  }

  const record = metadata as Record<string, unknown>;
  const timing = record.timing;
  if (timing && typeof timing === "object" && !Array.isArray(timing)) {
    return getValidTimingSource((timing as Record<string, unknown>).source);
  }

  return getValidTimingSource(record.timing_source);
}

function getBatchDurationSeconds(response: BatchResponse): number | undefined {
  const metadata = response.metadata;
  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata)) {
    return undefined;
  }

  const duration = (metadata as Record<string, unknown>).duration;
  return typeof duration === "number" &&
    Number.isFinite(duration) &&
    duration > 0
    ? duration
    : undefined;
}
