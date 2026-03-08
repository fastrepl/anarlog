import type { StoreApi } from "zustand";

import type { BatchResponse, StreamResponse } from "@hypr/plugin-listener2";

import type { HandlePersistCallback } from "./transcript";
import { transformWordEntries } from "./utils";

import {
  ChannelProfile,
  type RuntimeSpeakerHint,
  type WordLike,
} from "~/stt/segment";

export type BatchPhase = "importing" | "transcribing";

export type BatchState = {
  batch: Record<
    string,
    {
      percentage: number;
      isComplete?: boolean;
      error?: string;
      phase?: BatchPhase;
    }
  >;
  batchPersist: Record<string, HandlePersistCallback>;
};

export type BatchActions = {
  handleBatchStarted: (sessionId: string, phase?: BatchPhase) => void;
  handleBatchCompleted: (sessionId: string) => void;
  handleBatchResponse: (sessionId: string, response: BatchResponse) => void;
  handleBatchResponseStreamed: (
    sessionId: string,
    response: StreamResponse,
    percentage: number,
  ) => void;
  handleBatchFailed: (sessionId: string, error: string) => void;
  clearBatchSession: (sessionId: string) => void;
  setBatchPersist: (sessionId: string, callback: HandlePersistCallback) => void;
  clearBatchPersist: (sessionId: string) => void;
};

export const createBatchSlice = <T extends BatchState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): BatchState & BatchActions => ({
  batch: {},
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
        },
      },
    }));
  },

  handleBatchResponse: (sessionId, response) => {
    const persist = get().batchPersist[sessionId];

    const [words, hints] = transformBatch(response);
    if (!words.length) {
      return;
    }

    persist?.(words, hints);

    set((state) => {
      if (!state.batch[sessionId]) {
        return state;
      }

      const { [sessionId]: _, ...rest } = state.batch;
      return {
        ...state,
        batch: rest,
      };
    });
  },

  handleBatchResponseStreamed: (sessionId, response, percentage) => {
    const persist = get().batchPersist[sessionId];

    if (persist && response.type === "Results") {
      const channelIndex = response.channel_index[0];
      const alternative = response.channel.alternatives[0];

      if (channelIndex !== undefined && alternative) {
        const [words, hints] = transformWordEntries(
          alternative.words,
          alternative.transcript,
          channelIndex,
        );

        if (words.length > 0) {
          persist(words, hints);
        }
      }
    }

    const isComplete = response.type === "Results" && response.from_finalize;

    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          percentage,
          isComplete: isComplete || false,
          phase: "transcribing",
        },
      },
    }));
  },

  handleBatchFailed: (sessionId, error) => {
    set((state) => ({
      ...state,
      batch: {
        ...state.batch,
        [sessionId]: {
          ...(state.batch[sessionId] ?? { percentage: 0 }),
          error,
          isComplete: false,
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
      return {
        ...state,
        batch: rest,
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

function transformBatch(
  response: BatchResponse,
): [WordLike[], RuntimeSpeakerHint[]] {
  const allWords: WordLike[] = [];
  const allHints: RuntimeSpeakerHint[] = [];
  let wordOffset = 0;

  response.results.channels.forEach((channel) => {
    const alternative = channel.alternatives[0];
    if (!alternative || !alternative.words || !alternative.words.length) {
      return;
    }

    const [words, hints] = transformWordEntries(
      alternative.words,
      alternative.transcript,
      ChannelProfile.MixedCapture,
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
