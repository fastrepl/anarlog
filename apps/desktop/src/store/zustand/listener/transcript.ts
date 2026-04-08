import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import type {
  LiveTranscriptDelta,
  LiveTranscriptSegment,
  LiveTranscriptSegmentDelta,
} from "@hypr/plugin-transcription";

import type { RuntimeSpeakerHint, WordLike } from "~/stt/segment";

type WordsByChannel = Record<number, WordLike[]>;

export type BatchPersistCallback = (
  words: WordLike[],
  hints: RuntimeSpeakerHint[],
) => void;

export type LiveTranscriptPersistCallback = (
  delta: LiveTranscriptDelta,
) => void;

export type OnStoppedCallback = (
  sessionId: string,
  details: {
    durationSeconds: number;
    audioPath: string | null;
    requestedLiveTranscription: boolean;
    liveTranscriptionActive: boolean;
  },
) => void;

export type TranscriptState = {
  liveSegments: LiveTranscriptSegment[];
  liveSegmentsById: Record<string, LiveTranscriptSegment>;
  partialWordsByChannel: WordsByChannel;
  partialHintsByChannel: Record<number, RuntimeSpeakerHint[]>;
  handlePersistBySession: Record<string, LiveTranscriptPersistCallback>;
  onStoppedBySession: Record<string, OnStoppedCallback>;
};

export type TranscriptActions = {
  setTranscriptPersist: (
    sessionId: string,
    callback?: LiveTranscriptPersistCallback,
  ) => void;
  setOnStopped: (sessionId: string, callback?: OnStoppedCallback) => void;
  handleTranscriptDelta: (
    sessionId: string,
    delta: LiveTranscriptDelta,
  ) => void;
  handleTranscriptSegmentDelta: (delta: LiveTranscriptSegmentDelta) => void;
  takeOnStopped: (sessionId: string) => OnStoppedCallback | undefined;
  resetTranscript: () => void;
};

const initialState: TranscriptState = {
  liveSegments: [],
  liveSegmentsById: {},
  partialWordsByChannel: {},
  partialHintsByChannel: {},
  handlePersistBySession: {},
  onStoppedBySession: {},
};

export const createTranscriptSlice = <
  T extends TranscriptState & TranscriptActions,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): TranscriptState & TranscriptActions => ({
  ...initialState,
  setTranscriptPersist: (sessionId, callback) => {
    set((state) =>
      mutate(state, (draft) => {
        if (callback) {
          draft.handlePersistBySession[sessionId] = callback;
        } else {
          delete draft.handlePersistBySession[sessionId];
        }
      }),
    );
  },
  setOnStopped: (sessionId, callback) => {
    set((state) =>
      mutate(state, (draft) => {
        if (callback) {
          draft.onStoppedBySession[sessionId] = callback;
        } else {
          delete draft.onStoppedBySession[sessionId];
        }
      }),
    );
  },
  handleTranscriptDelta: (sessionId, delta) => {
    const handlePersist = get().handlePersistBySession[sessionId];
    const { wordsByChannel, hintsByChannel } = groupPartialsByChannel(
      delta.partials,
    );

    set((state) =>
      mutate(state, (draft) => {
        draft.partialWordsByChannel = wordsByChannel;
        draft.partialHintsByChannel = hintsByChannel;
      }),
    );

    if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
      return;
    }

    handlePersist?.(delta);
  },
  handleTranscriptSegmentDelta: (delta) => {
    set((state) =>
      mutate(state, (draft) => {
        for (const removedId of delta.removed_ids) {
          delete draft.liveSegmentsById[removedId];
        }
        for (const segment of delta.upserts) {
          draft.liveSegmentsById[segment.id] = segment;
        }
        draft.liveSegments = Object.values(draft.liveSegmentsById).sort(
          (a, b) => a.start_ms - b.start_ms,
        );
      }),
    );
  },
  takeOnStopped: (sessionId) => {
    const callback = get().onStoppedBySession[sessionId];
    set((state) =>
      mutate(state, (draft) => {
        delete draft.onStoppedBySession[sessionId];
        delete draft.handlePersistBySession[sessionId];
      }),
    );
    return callback;
  },
  resetTranscript: () => {
    set((state) =>
      mutate(state, (draft) => {
        draft.liveSegments = [];
        draft.liveSegmentsById = {};
        draft.partialWordsByChannel = {};
        draft.partialHintsByChannel = {};
      }),
    );
  },
});

function groupPartialsByChannel(partials: LiveTranscriptDelta["partials"]): {
  wordsByChannel: WordsByChannel;
  hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
} {
  const wordsByChannel: WordsByChannel = {};
  const hintsByChannel: Record<number, RuntimeSpeakerHint[]> = {};

  partials.forEach((word) => {
    const channel = word.channel;
    const channelWords = wordsByChannel[channel] ?? [];
    if (!(channel in wordsByChannel)) {
      wordsByChannel[channel] = channelWords;
      hintsByChannel[channel] = [];
    }

    const channelIndex = channelWords.length;
    channelWords.push(word);

    if (word.speaker_index != null) {
      hintsByChannel[channel]!.push({
        wordIndex: channelIndex,
        data: {
          type: "provider_speaker_index",
          speaker_index: word.speaker_index,
          channel,
        },
      });
    }
  });

  return { wordsByChannel, hintsByChannel };
}
