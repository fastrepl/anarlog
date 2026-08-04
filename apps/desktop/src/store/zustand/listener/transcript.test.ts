import { beforeEach, describe, expect, test, vi } from "vitest";
import { createStore } from "zustand";

import type {
  LiveTranscriptDelta,
  LiveTranscriptSegment,
} from "@anlg/plugin-transcription";

import {
  createTranscriptSlice,
  LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT,
  type TranscriptActions,
  type TranscriptState,
} from "./transcript";

const createTranscriptStore = () => {
  return createStore<TranscriptState & TranscriptActions>((set, get) =>
    createTranscriptSlice(set, get),
  );
};

describe("transcript slice", () => {
  type TranscriptStore = ReturnType<typeof createTranscriptStore>;
  let store: TranscriptStore;

  beforeEach(() => {
    store = createTranscriptStore();
  });

  const createDelta = (
    speakerIndices: Record<number, number> = {},
  ): LiveTranscriptDelta => ({
    new_words: [],
    replaced_ids: [],
    partials: [
      {
        text: " hello",
        start_ms: 0,
        end_ms: 100,
        channel: 0,
        speaker_index: speakerIndices[0] ?? null,
      },
      {
        text: " remote",
        start_ms: 200,
        end_ms: 300,
        channel: 1,
        speaker_index: speakerIndices[1] ?? null,
      },
      {
        text: " again",
        start_ms: 350,
        end_ms: 450,
        channel: 1,
        speaker_index: speakerIndices[2] ?? null,
      },
    ],
  });

  test("groups partial snapshot by channel and reindexes hints", () => {
    store
      .getState()
      .handleTranscriptDelta("session-1", createDelta({ 0: 0, 2: 1 }));

    expect(
      store.getState().partialWordsByChannel[0]?.map((word) => word.text),
    ).toEqual([" hello"]);
    expect(
      store.getState().partialWordsByChannel[1]?.map((word) => word.text),
    ).toEqual([" remote", " again"]);

    expect(store.getState().partialHintsByChannel[0]).toEqual([
      {
        wordIndex: 0,
        data: {
          type: "provider_speaker_index",
          speaker_index: 0,
          channel: 0,
        },
      },
    ]);
    expect(store.getState().partialHintsByChannel[1]).toEqual([
      {
        wordIndex: 1,
        data: {
          type: "provider_speaker_index",
          speaker_index: 1,
          channel: 1,
        },
      },
    ]);
  });

  test("forwards persisted transcript deltas to the callback", () => {
    const persist = vi.fn();
    store.getState().setTranscriptPersist("session-1", persist);

    const delta: LiveTranscriptDelta = {
      new_words: [
        {
          id: "word-1",
          text: " hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          state: "final",
          speaker_index: 0,
        },
      ],
      replaced_ids: ["old-word"],
      partials: [],
    };

    store.getState().handleTranscriptDelta("session-1", delta);

    expect(persist).toHaveBeenCalledTimes(1);
    expect(persist).toHaveBeenCalledWith(delta);
    expect(store.getState().partialWordsByChannel).toEqual({});
    expect(store.getState().partialHintsByChannel).toEqual({});
    expect(store.getState().liveCaptionText).toBe("hello");
  });

  test("uses partial words for the live caption text", () => {
    store.getState().handleTranscriptDelta("session-1", createDelta());

    expect(store.getState().liveCaptionText).toBe("hello remote again");
  });

  test("replaces longer partial caption text with shorter finalized words", () => {
    store.getState().handleTranscriptDelta("session-1", createDelta());
    store.getState().handleTranscriptDelta("session-1", {
      new_words: [
        {
          id: "word-2",
          text: " done",
          start_ms: 200,
          end_ms: 300,
          channel: 1,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    expect(store.getState().liveCaptionText).toBe("done");
  });

  test("appends delayed partials after finalized caption text", () => {
    store.getState().handleTranscriptDelta("session-1", {
      new_words: [
        {
          id: "word-1",
          text: " hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
        {
          id: "word-2",
          text: " there",
          start_ms: 120,
          end_ms: 220,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    store.getState().handleTranscriptDelta("session-1", {
      new_words: [],
      replaced_ids: [],
      partials: [
        {
          text: " checking",
          start_ms: 900,
          end_ms: 1000,
          channel: 0,
          speaker_index: null,
        },
        {
          text: " again",
          start_ms: 1020,
          end_ms: 1120,
          channel: 0,
          speaker_index: null,
        },
      ],
    });

    expect(store.getState().liveCaptionText).toBe("hello there checking again");
  });

  test("appends delayed finalized words after finalized caption text", () => {
    store.getState().handleTranscriptDelta("session-1", {
      new_words: [
        {
          id: "word-1",
          text: " hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    store.getState().handleTranscriptDelta("session-1", {
      new_words: [
        {
          id: "word-2",
          text: " again",
          start_ms: 800,
          end_ms: 900,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    expect(store.getState().liveCaptionText).toBe("hello again");
  });

  test("keeps the previous live caption text when a delta has no words", () => {
    store.getState().handleTranscriptDelta("session-1", createDelta());
    store.getState().handleTranscriptDelta("session-1", {
      new_words: [],
      replaced_ids: [],
      partials: [],
    });

    expect(store.getState().liveCaptionText).toBe("hello remote again");
  });

  test("bounds retained finalized words to the live-caption history", () => {
    store.getState().handleTranscriptDelta("session-1", {
      new_words: Array.from({ length: 1000 }, (_, index) => ({
        id: `word-${index}`,
        text: ` word-${index}`,
        start_ms: index * 100,
        end_ms: index * 100 + 50,
        channel: 0,
        state: "final" as const,
        speaker_index: null,
      })),
      replaced_ids: [],
      partials: [],
    });

    expect(
      Object.keys(store.getState().liveCaptionFinalWordsById).length,
    ).toBeLessThan(1000);
    expect(store.getState().liveCaptionText.length).toBeLessThanOrEqual(2048);
    expect(store.getState().liveCaptionText).toContain("word-999");
  });

  test("bounds one giant finalized word without splitting Unicode", () => {
    const originalText = ` prefix ${"🙂".repeat(3000)}`;
    store.getState().handleTranscriptDelta("session-1", {
      new_words: [
        {
          id: "giant-word",
          text: originalText,
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    const retained =
      store.getState().liveCaptionFinalWordsById["giant-word"]?.text;
    expect(retained?.length).toBeLessThanOrEqual(2048);
    expect(retained).not.toBe(originalText);
    expect([...(retained ?? "")].every((character) => character === "🙂")).toBe(
      true,
    );
  });

  test("bounds finalized-word state even when provider words are empty", () => {
    store.getState().handleTranscriptDelta("session-1", {
      new_words: Array.from({ length: 3000 }, (_, index) => ({
        id: `empty-${index}`,
        text: "",
        start_ms: index,
        end_ms: index + 1,
        channel: 0,
        state: "final" as const,
        speaker_index: null,
      })),
      replaced_ids: [],
      partials: [],
    });

    expect(
      Object.keys(store.getState().liveCaptionFinalWordsById),
    ).toHaveLength(2048);
    expect(store.getState().liveCaptionFinalWordsById).toHaveProperty(
      "empty-2999",
    );
    expect(store.getState().liveCaptionFinalWordsById).not.toHaveProperty(
      "empty-0",
    );
  });

  test("applies segment deltas without retaining a second segment index", () => {
    const segment = (id: string, startMs: number): LiveTranscriptSegment => ({
      id,
      key: {
        channel: "DirectMic",
        speaker_index: null,
        speaker_human_id: null,
      },
      start_ms: startMs,
      end_ms: startMs + 100,
      text: id,
      words: [],
    });

    store.getState().handleTranscriptSegmentDelta({
      upserts: [segment("later", 200), segment("earlier", 100)],
      removed_ids: [],
    });
    store.getState().handleTranscriptSegmentDelta({
      upserts: [segment("later", 150)],
      removed_ids: ["earlier"],
    });

    expect(store.getState().liveSegments).toEqual([segment("later", 150)]);
    expect(store.getState()).not.toHaveProperty("liveSegmentsById");
  });

  test("bounds the retained segment preview independently of persistence", () => {
    const segmentCount = LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT + 5;
    const segments: LiveTranscriptSegment[] = Array.from(
      { length: segmentCount },
      (_, index) => ({
        id: `segment-${index}`,
        key: {
          channel: "DirectMic",
          speaker_index: null,
          speaker_human_id: null,
        },
        start_ms: index * 100,
        end_ms: index * 100 + 50,
        text: `segment ${index}`,
        words: [],
      }),
    );

    store.getState().handleTranscriptSegmentDelta({
      upserts: segments,
      removed_ids: [],
    });

    expect(store.getState().liveSegments).toHaveLength(
      LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT,
    );
    expect(store.getState().liveSegments[0]?.id).toBe("segment-5");
  });

  test("can persist deltas without replacing the active live preview", () => {
    const persist = vi.fn();
    store.getState().setTranscriptPersist("session-1", persist);
    store.getState().handleTranscriptDelta("active-session", createDelta());

    const delta: LiveTranscriptDelta = {
      new_words: [
        {
          id: "word-1",
          text: " background",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    };

    store.getState().handleTranscriptDelta("session-1", delta, {
      updateLivePreview: false,
    });

    expect(persist).toHaveBeenCalledWith(delta);
    expect(
      store.getState().partialWordsByChannel[0]?.map((word) => word.text),
    ).toEqual([" hello"]);
    expect(store.getState().liveCaptionText).toBe("hello remote again");
  });

  test("resetTranscript clears partial state and callbacks", () => {
    store.getState().setTranscriptPersist("session-1", vi.fn());
    store.getState().setOnStopped("session-1", vi.fn());
    store.getState().handleTranscriptDelta("session-1", createDelta());

    store.getState().resetTranscript();

    expect(store.getState().partialWordsByChannel).toEqual({});
    expect(store.getState().partialHintsByChannel).toEqual({});
    expect(store.getState().liveCaptionText).toBe("");
    expect(store.getState().handlePersistBySession).toEqual({
      "session-1": expect.any(Function),
    });
    expect(store.getState().onStoppedBySession).toEqual({
      "session-1": expect.any(Function),
    });
  });
});
