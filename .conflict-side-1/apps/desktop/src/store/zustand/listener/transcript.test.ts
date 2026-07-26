import { beforeEach, describe, expect, test, vi } from "vitest";
import { createStore } from "zustand";

import type { LiveTranscriptDelta } from "@hypr/plugin-transcription";

import {
  createTranscriptSlice,
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
