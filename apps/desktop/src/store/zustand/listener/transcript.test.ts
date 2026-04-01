import { beforeEach, describe, expect, test, vi } from "vitest";
import { createStore } from "zustand";

import type { LiveTranscriptDelta } from "@hypr/plugin-listener";

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
    store.getState().handleTranscriptDelta(createDelta({ 0: 0, 2: 1 }));

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
    store.getState().setTranscriptPersist(persist);

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

    store.getState().handleTranscriptDelta(delta);

    expect(persist).toHaveBeenCalledTimes(1);
    expect(persist).toHaveBeenCalledWith(delta);
    expect(store.getState().partialWordsByChannel).toEqual({});
    expect(store.getState().partialHintsByChannel).toEqual({});
  });

  test("resetTranscript clears partial state and callbacks", () => {
    store.getState().setTranscriptPersist(vi.fn());
    store.getState().setOnStopped(vi.fn());
    store.getState().handleTranscriptDelta(createDelta());

    store.getState().resetTranscript();

    expect(store.getState().partialWordsByChannel).toEqual({});
    expect(store.getState().partialHintsByChannel).toEqual({});
    expect(store.getState().handlePersist).toBeUndefined();
    expect(store.getState().onStopped).toBeUndefined();
  });
});
