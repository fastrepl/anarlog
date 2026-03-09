import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import type { StreamResponse } from "@hypr/plugin-listener";

import { transformWordEntries } from "./utils";

import type { RuntimeSpeakerHint, WordLike } from "~/stt/segment";

type WordsByChannel = Record<number, WordLike[]>;

export type HandlePersistCallback = (
  words: WordLike[],
  hints: RuntimeSpeakerHint[],
) => void;

export type TranscriptState = {
  finalWordsMaxEndMsByChannel: Record<number, number>;
  partialWordsByChannel: WordsByChannel;
  partialHintsByChannel: Record<number, RuntimeSpeakerHint[]>;
  handlePersist?: HandlePersistCallback;
};

export type TranscriptActions = {
  setTranscriptPersist: (callback?: HandlePersistCallback) => void;
  handleTranscriptResponse: (response: StreamResponse) => void;
  resetTranscript: () => void;
};

const initialState: TranscriptState = {
  finalWordsMaxEndMsByChannel: {},
  partialWordsByChannel: {},
  partialHintsByChannel: {},
  handlePersist: undefined,
};

export const createTranscriptSlice = <
  T extends TranscriptState & TranscriptActions,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): TranscriptState & TranscriptActions => {
  const handleFinalWords = (
    channelIndex: number,
    words: WordLike[],
    hints: RuntimeSpeakerHint[],
  ): void => {
    const {
      partialWordsByChannel,
      partialHintsByChannel,
      handlePersist,
      finalWordsMaxEndMsByChannel,
    } = get();

    const lastPersistedEndMs = finalWordsMaxEndMsByChannel[channelIndex] ?? 0;
    const lastEndMs = getLastEndMs(words);

    const firstNewWordIndex = words.findIndex(
      (word) => word.end_ms > lastPersistedEndMs,
    );
    if (firstNewWordIndex === -1) {
      return;
    }

    const newWords = words.slice(firstNewWordIndex);
    const newHints = hints
      .filter((hint) => hint.wordIndex >= firstNewWordIndex)
      .map((hint) => ({
        ...hint,
        wordIndex: hint.wordIndex - firstNewWordIndex,
      }));

    const existingPartialWords = partialWordsByChannel[channelIndex] ?? [];
    const remainingPartialWords = existingPartialWords.filter(
      (word) => word.start_ms > lastEndMs,
    );

    const oldToNewIndex = new Map<number, number>();
    let newIdx = 0;
    for (let oldIdx = 0; oldIdx < existingPartialWords.length; oldIdx++) {
      if (existingPartialWords[oldIdx].start_ms > lastEndMs) {
        oldToNewIndex.set(oldIdx, newIdx);
        newIdx++;
      }
    }

    const existingPartialHints = partialHintsByChannel[channelIndex] ?? [];
    const remainingPartialHints = existingPartialHints
      .filter((hint) => oldToNewIndex.has(hint.wordIndex))
      .map((hint) => ({
        ...hint,
        wordIndex: oldToNewIndex.get(hint.wordIndex)!,
      }));

    set((state) =>
      mutate(state, (draft) => {
        draft.partialWordsByChannel[channelIndex] = remainingPartialWords;
        draft.partialHintsByChannel[channelIndex] = remainingPartialHints;
        draft.finalWordsMaxEndMsByChannel[channelIndex] = lastEndMs;
      }),
    );

    handlePersist?.(newWords, newHints);
  };

  const handlePartialWords = (
    channelIndex: number,
    words: WordLike[],
    hints: RuntimeSpeakerHint[],
  ): void => {
    const { partialWordsByChannel, partialHintsByChannel } = get();
    const existing = partialWordsByChannel[channelIndex] ?? [];

    const firstStartMs = getFirstStartMs(words);
    const lastEndMs = getLastEndMs(words);

    const [before, after] = [
      existing.filter((word) => word.end_ms <= firstStartMs),
      existing.filter((word) => word.start_ms >= lastEndMs),
    ];

    const newWords = [...before, ...words, ...after];

    const hintsWithAdjustedIndices = hints.map((hint) => ({
      ...hint,
      wordIndex: before.length + hint.wordIndex,
    }));

    const existingHints = partialHintsByChannel[channelIndex] ?? [];
    const filteredOldHints = existingHints.filter((hint) => {
      const word = existing[hint.wordIndex];
      return (
        word && (word.end_ms <= firstStartMs || word.start_ms >= lastEndMs)
      );
    });

    set((state) =>
      mutate(state, (draft) => {
        draft.partialWordsByChannel[channelIndex] = newWords;
        draft.partialHintsByChannel[channelIndex] = [
          ...filteredOldHints,
          ...hintsWithAdjustedIndices,
        ];
      }),
    );
  };

  return {
    ...initialState,
    setTranscriptPersist: (callback) => {
      set((state) =>
        mutate(state, (draft) => {
          draft.handlePersist = callback;
        }),
      );
    },
    handleTranscriptResponse: (response) => {
      if (response.type !== "Results") {
        return;
      }

      const channelIndex = response.channel_index[0];
      const alternative = response.channel.alternatives[0];
      if (channelIndex === undefined || !alternative) {
        return;
      }

      const [words, hints] = transformWordEntries(
        alternative.words,
        alternative.transcript,
        channelIndex,
      );
      if (!words.length) {
        return;
      }

      if (response.is_final) {
        handleFinalWords(channelIndex, words, hints);
      } else {
        handlePartialWords(channelIndex, words, hints);
      }
    },
    resetTranscript: () => {
      const { partialWordsByChannel, partialHintsByChannel, handlePersist } =
        get();

      const remainingWords = Object.values(partialWordsByChannel).flat();

      const channelIndices = Object.keys(partialWordsByChannel)
        .map(Number)
        .sort((a, b) => a - b);

      const offsetByChannel = new Map<number, number>();
      let currentOffset = 0;
      for (const channelIndex of channelIndices) {
        offsetByChannel.set(channelIndex, currentOffset);
        currentOffset += partialWordsByChannel[channelIndex]?.length ?? 0;
      }

      const remainingHints: RuntimeSpeakerHint[] = [];
      for (const channelIndex of channelIndices) {
        const hints = partialHintsByChannel[channelIndex] ?? [];
        const offset = offsetByChannel.get(channelIndex) ?? 0;
        for (const hint of hints) {
          remainingHints.push({
            ...hint,
            wordIndex: hint.wordIndex + offset,
          });
        }
      }

      if (remainingWords.length > 0) {
        handlePersist?.(remainingWords, remainingHints);
      }

      set((state) =>
        mutate(state, (draft) => {
          draft.partialWordsByChannel = {};
          draft.partialHintsByChannel = {};
          draft.finalWordsMaxEndMsByChannel = {};
          draft.handlePersist = undefined;
        }),
      );
    },
  };
};

const getLastEndMs = (words: WordLike[]): number =>
  words[words.length - 1]?.end_ms ?? 0;
const getFirstStartMs = (words: WordLike[]): number => words[0]?.start_ms ?? 0;
