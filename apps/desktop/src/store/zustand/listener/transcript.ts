import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import type {
  LiveTranscriptDelta,
  LiveTranscriptSegment,
  LiveTranscriptSegmentDelta,
  PartialSpeakerHint,
  SpeakerHintData,
} from "@hypr/plugin-listener";

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
  durationSeconds: number,
) => void;

export type TranscriptState = {
  liveSegments: LiveTranscriptSegment[];
  liveSegmentsById: Record<string, LiveTranscriptSegment>;
  partialWordsByChannel: WordsByChannel;
  partialHintsByChannel: Record<number, RuntimeSpeakerHint[]>;
  handlePersist?: LiveTranscriptPersistCallback;
  onStopped?: OnStoppedCallback;
};

export type TranscriptActions = {
  setTranscriptPersist: (callback?: LiveTranscriptPersistCallback) => void;
  setOnStopped: (callback?: OnStoppedCallback) => void;
  handleTranscriptDelta: (delta: LiveTranscriptDelta) => void;
  handleTranscriptSegmentDelta: (delta: LiveTranscriptSegmentDelta) => void;
  resetTranscript: () => void;
};

const initialState: TranscriptState = {
  liveSegments: [],
  liveSegmentsById: {},
  partialWordsByChannel: {},
  partialHintsByChannel: {},
  handlePersist: undefined,
  onStopped: undefined,
};

export const createTranscriptSlice = <
  T extends TranscriptState & TranscriptActions,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): TranscriptState & TranscriptActions => ({
  ...initialState,
  setTranscriptPersist: (callback) => {
    set((state) =>
      mutate(state, (draft) => {
        draft.handlePersist = callback;
      }),
    );
  },
  setOnStopped: (callback) => {
    set((state) =>
      mutate(state, (draft) => {
        draft.onStopped = callback;
      }),
    );
  },
  handleTranscriptDelta: (delta) => {
    const { handlePersist } = get();
    const { wordsByChannel, hintsByChannel } = groupPartialsByChannel(
      delta.partials,
      delta.partial_hints,
    );

    set((state) =>
      mutate(state, (draft) => {
        draft.partialWordsByChannel = wordsByChannel;
        draft.partialHintsByChannel = hintsByChannel;
      }),
    );

    if (
      delta.new_words.length === 0 &&
      delta.hints.length === 0 &&
      delta.replaced_ids.length === 0
    ) {
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
  resetTranscript: () => {
    set((state) =>
      mutate(state, (draft) => {
        draft.liveSegments = [];
        draft.liveSegmentsById = {};
        draft.partialWordsByChannel = {};
        draft.partialHintsByChannel = {};
        draft.handlePersist = undefined;
        draft.onStopped = undefined;
      }),
    );
  },
});

function groupPartialsByChannel(
  partials: LiveTranscriptDelta["partials"],
  partialHints: LiveTranscriptDelta["partial_hints"],
): {
  wordsByChannel: WordsByChannel;
  hintsByChannel: Record<number, RuntimeSpeakerHint[]>;
} {
  const wordsByChannel: WordsByChannel = {};
  const hintsByChannel: Record<number, RuntimeSpeakerHint[]> = {};

  const offsets = new Map<number, number>();
  partials.forEach((word) => {
    if (!(word.channel in wordsByChannel)) {
      wordsByChannel[word.channel] = [];
      offsets.set(word.channel, 0);
    }
    wordsByChannel[word.channel]!.push(word);
  });

  let globalIndex = 0;
  for (const [channelKey, words] of Object.entries(wordsByChannel)) {
    offsets.set(Number(channelKey), globalIndex);
    globalIndex += words.length;
  }

  partialHints.forEach((hint) => {
    const partial = partials[hint.word_index];
    if (!partial) {
      return;
    }

    const channel = partial.channel;
    const channelOffset = offsets.get(channel) ?? 0;
    if (!(channel in hintsByChannel)) {
      hintsByChannel[channel] = [];
    }

    hintsByChannel[channel]!.push(toRuntimeSpeakerHint(hint, channelOffset));
  });

  return { wordsByChannel, hintsByChannel };
}

function toRuntimeSpeakerHint(
  hint: PartialSpeakerHint,
  channelOffset: number,
): RuntimeSpeakerHint {
  return {
    wordIndex: hint.word_index - channelOffset,
    data: toRuntimeSpeakerHintData(hint.data),
  };
}

function toRuntimeSpeakerHintData(
  data: SpeakerHintData,
): RuntimeSpeakerHint["data"] {
  if ("provider_speaker_index" in data) {
    return {
      type: "provider_speaker_index",
      speaker_index: data.provider_speaker_index.speaker_index,
      provider: data.provider_speaker_index.provider ?? undefined,
      channel: data.provider_speaker_index.channel ?? undefined,
    };
  }

  return {
    type: "user_speaker_assignment",
    human_id: data.user_speaker_assignment.human_id,
  };
}
