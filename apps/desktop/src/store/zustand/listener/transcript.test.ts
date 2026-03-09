import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createStore } from "zustand";

import type { StreamResponse, StreamWord } from "@hypr/plugin-listener";

import {
  createTranscriptSlice,
  type TranscriptActions,
  type TranscriptState,
} from "./transcript";

import type { RuntimeSpeakerHint, WordLike } from "~/stt/segment";

const createTranscriptStore = () => {
  return createStore<TranscriptState & TranscriptActions>((set, get) =>
    createTranscriptSlice(set, get),
  );
};

describe("transcript slice", () => {
  const defaultWords: StreamWord[] = [
    {
      word: "another",
      punctuated_word: "Another",
      start: 0,
      end: 1,
      confidence: 1,
      speaker: 0,
      language: "en",
    },
    {
      word: "problem",
      punctuated_word: "problem",
      start: 1,
      end: 2,
      confidence: 1,
      speaker: 1,
      language: "en",
    },
  ];

  const createResponse = ({
    words,
    transcript,
    isFinal,
    channelIndex = 0,
  }: {
    words: StreamWord[];
    transcript: string;
    isFinal: boolean;
    channelIndex?: number;
  }): StreamResponse => {
    return {
      type: "Results",
      start: 0,
      duration: 0,
      is_final: isFinal,
      speech_final: isFinal,
      from_finalize: false,
      channel_index: [channelIndex],
      channel: {
        alternatives: [
          {
            transcript,
            confidence: 1,
            words,
          },
        ],
      },
      metadata: {
        request_id: "test",
        model_info: { name: "model", version: "1", arch: "cpu" },
        model_uuid: "model",
      },
    } satisfies StreamResponse;
  };

  type TranscriptStore = ReturnType<typeof createTranscriptStore>;
  let store: TranscriptStore;

  beforeEach(() => {
    store = createTranscriptStore();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("stores partial words and hints from streaming updates", () => {
    const initialPartial = createResponse({
      words: defaultWords,
      transcript: "Another problem",
      isFinal: false,
    });

    store.getState().handleTranscriptResponse(initialPartial);

    const stateAfterFirst = store.getState();
    const firstChannelWords = stateAfterFirst.partialWordsByChannel[0];
    expect(firstChannelWords).toHaveLength(2);
    expect(firstChannelWords?.map((word) => word.text)).toEqual([
      " Another",
      " problem",
    ]);
    expect(stateAfterFirst.partialHintsByChannel[0]).toHaveLength(2);
    expect(stateAfterFirst.partialHintsByChannel[0]?.[0]?.wordIndex).toBe(0);
    expect(stateAfterFirst.partialHintsByChannel[0]?.[1]?.wordIndex).toBe(1);

    const extendedPartial = createResponse({
      words: [
        ...defaultWords,
        {
          word: "exists",
          punctuated_word: "exists",
          start: 2,
          end: 3,
          confidence: 1,
          speaker: 1,
          language: "en",
        },
      ],
      transcript: "Another problem exists",
      isFinal: false,
    });

    store.getState().handleTranscriptResponse(extendedPartial);

    const stateAfterSecond = store.getState();
    const updatedWords = stateAfterSecond.partialWordsByChannel[0];
    expect(updatedWords).toHaveLength(3);
    expect(updatedWords?.map((word) => word.text)).toEqual([
      " Another",
      " problem",
      " exists",
    ]);
    const channelHints = stateAfterSecond.partialHintsByChannel[0] ?? [];
    expect(channelHints).toHaveLength(3);
    const lastPartialHint = channelHints[channelHints.length - 1];
    expect(lastPartialHint?.wordIndex).toBe(2);
  });

  test("persists only new final words", () => {
    const persist = vi.fn();
    store.getState().setTranscriptPersist(persist);

    const finalResponse = createResponse({
      words: [
        {
          word: "hello",
          punctuated_word: "Hello",
          start: 0,
          end: 0.5,
          confidence: 1,
          speaker: 0,
          language: "en",
        },
        {
          word: "world",
          punctuated_word: "world",
          start: 0.5,
          end: 1.5,
          confidence: 1,
          speaker: null,
          language: "en",
        },
      ],
      transcript: "Hello world",
      isFinal: true,
    });

    store.getState().handleTranscriptResponse(finalResponse);
    expect(persist).toHaveBeenCalledTimes(1);

    const [words, hints] = persist.mock.calls[0] as [
      WordLike[],
      RuntimeSpeakerHint[],
    ];
    expect(words.map((word) => word.text)).toEqual([" Hello", " world"]);
    expect(words.map((word) => word.end_ms)).toEqual([500, 1500]);
    expect(hints).toEqual([
      {
        data: { type: "provider_speaker_index", speaker_index: 0 },
        wordIndex: 0,
      },
    ]);

    store.getState().handleTranscriptResponse(finalResponse);
    expect(persist).toHaveBeenCalledTimes(1);
    expect(store.getState().finalWordsMaxEndMsByChannel[0]).toBe(1500);
  });

  test("adjusts partial hint indices after filtering partial words", () => {
    const persist = vi.fn();
    store.getState().setTranscriptPersist(persist);

    const partialResponse = createResponse({
      words: [
        {
          word: "hello",
          punctuated_word: "Hello",
          start: 0,
          end: 0.5,
          confidence: 1,
          speaker: 0,
          language: "en",
        },
        {
          word: "world",
          punctuated_word: "world",
          start: 0.5,
          end: 1.0,
          confidence: 1,
          speaker: 1,
          language: "en",
        },
        {
          word: "test",
          punctuated_word: "test",
          start: 1.1,
          end: 1.5,
          confidence: 1,
          speaker: 0,
          language: "en",
        },
      ],
      transcript: "Hello world test",
      isFinal: false,
    });

    store.getState().handleTranscriptResponse(partialResponse);

    const stateAfterPartial = store.getState();
    expect(stateAfterPartial.partialWordsByChannel[0]).toHaveLength(3);
    expect(stateAfterPartial.partialHintsByChannel[0]).toHaveLength(3);

    const finalResponse = createResponse({
      words: [
        {
          word: "hello",
          punctuated_word: "Hello",
          start: 0,
          end: 0.5,
          confidence: 1,
          speaker: 0,
          language: "en",
        },
        {
          word: "world",
          punctuated_word: "world",
          start: 0.5,
          end: 1.0,
          confidence: 1,
          speaker: 1,
          language: "en",
        },
      ],
      transcript: "Hello world",
      isFinal: true,
    });

    store.getState().handleTranscriptResponse(finalResponse);

    const stateAfterFinal = store.getState();
    const remainingPartialWords = stateAfterFinal.partialWordsByChannel[0];
    const remainingHints = stateAfterFinal.partialHintsByChannel[0] ?? [];

    expect(remainingPartialWords).toHaveLength(1);
    expect(remainingPartialWords?.[0]?.text).toBe(" test");

    expect(remainingHints).toHaveLength(1);
    expect(remainingHints[0]?.wordIndex).toBe(0);

    const hintedWord =
      remainingPartialWords?.[remainingHints[0]?.wordIndex ?? -1];
    expect(hintedWord).toBeDefined();
    expect(hintedWord?.text).toBe(" test");
  });
});
