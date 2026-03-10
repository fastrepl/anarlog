import { create as mutate } from "mutative";
import { beforeEach, describe, expect, test } from "vitest";

import { createListenerStore } from ".";

let store: ReturnType<typeof createListenerStore>;

describe("General Listener Slice", () => {
  beforeEach(() => {
    store = createListenerStore();
  });

  describe("Initial State", () => {
    test("initializes with correct default values", () => {
      const state = store.getState();
      expect(state.live.status).toBe("inactive");
      expect(state.live.loading).toBe(false);
      expect(state.live.amplitude).toEqual({ mic: 0, speaker: 0 });
      expect(state.live.seconds).toBe(0);
      expect(state.live.eventUnlisteners).toBeUndefined();
      expect(state.live.intervalId).toBeUndefined();
      expect(state.batch).toEqual({});
    });
  });

  describe("Amplitude Updates", () => {
    test("amplitude state is initialized to zero", () => {
      const state = store.getState();
      expect(state.live.amplitude).toEqual({ mic: 0, speaker: 0 });
    });
  });

  describe("Session Mode Helpers", () => {
    test("getSessionMode defaults to inactive", () => {
      const state = store.getState();
      expect(state.getSessionMode("session-123")).toBe("inactive");
    });

    test("getSessionMode returns running_batch when session is in batch", () => {
      const sessionId = "session-456";
      const { handleBatchResponseStreamed, getSessionMode } = store.getState();

      const mockResponse = {
        type: "Results" as const,
        start: 0,
        duration: 5,
        is_final: false,
        speech_final: false,
        from_finalize: false,
        channel: {
          alternatives: [
            {
              transcript: "test",
              words: [],
              confidence: 0.9,
            },
          ],
        },
        metadata: {
          request_id: "test-request",
          model_info: {
            name: "test-model",
            version: "1.0",
            arch: "test-arch",
          },
          model_uuid: "test-uuid",
        },
        channel_index: [0],
      };

      handleBatchResponseStreamed(sessionId, mockResponse, 0.5);
      expect(getSessionMode(sessionId)).toBe("running_batch");
    });
  });

  describe("Batch State", () => {
    test("handleBatchResponseStreamed tracks progress per session", () => {
      const sessionId = "session-progress";
      const { handleBatchResponseStreamed, clearBatchSession } =
        store.getState();

      const mockResponse = {
        type: "Results" as const,
        start: 0,
        duration: 5,
        is_final: false,
        speech_final: false,
        from_finalize: false,
        channel: {
          alternatives: [
            {
              transcript: "test",
              languages: [],
              words: [
                {
                  word: "test",
                  punctuated_word: "test",
                  start: 0,
                  end: 0.5,
                  confidence: 0.9,
                  speaker: null,
                  language: null,
                },
              ],
              confidence: 0.9,
            },
          ],
        },
        metadata: {
          request_id: "test-request",
          model_info: {
            name: "test-model",
            version: "1.0",
            arch: "test-arch",
          },
          model_uuid: "test-uuid",
        },
        channel_index: [0],
      };

      handleBatchResponseStreamed(sessionId, mockResponse, 0.5);
      expect(store.getState().batch[sessionId]).toEqual({
        percentage: 0.5,
        isComplete: false,
        phase: "transcribing",
      });
      expect(
        store.getState().batchPreview[sessionId]?.wordsByChannel[0],
      ).toEqual([
        {
          text: " test",
          start_ms: 0,
          end_ms: 500,
          channel: 0,
        },
      ]);

      clearBatchSession(sessionId);
      expect(store.getState().batch[sessionId]).toBeUndefined();
      expect(store.getState().batchPreview[sessionId]).toBeUndefined();
    });

    test("handleBatchFailed preserves batch error for UI surfaces", () => {
      const sessionId = "session-batch-error";
      const { handleBatchFailed, getSessionMode } = store.getState();

      handleBatchFailed(sessionId, "batch start failed: connection refused");

      expect(store.getState().batch[sessionId]).toEqual({
        percentage: 0,
        error: "batch start failed: connection refused",
        isComplete: false,
      });
      expect(getSessionMode(sessionId)).toBe("inactive");
    });
  });

  describe("Stop Action", () => {
    test("stop action exists and is callable", () => {
      const stop = store.getState().stop;
      expect(typeof stop).toBe("function");
    });
  });

  describe("Start Action", () => {
    test("start action exists and is callable", () => {
      const start = store.getState().start;
      expect(typeof start).toBe("function");
    });

    test("start returns false while another session is finalizing", async () => {
      store.setState((state) =>
        mutate(state, (draft) => {
          draft.live.status = "finalizing";
          draft.live.loading = true;
          draft.live.sessionId = "session-a";
        }),
      );

      const result = await store.getState().start({
        session_id: "session-b",
        languages: [],
        onboarding: false,
        transcription_mode: "live",
        recording_mode: "disk",
        model: "test-model",
        base_url: "http://localhost",
        api_key: "test-key",
        keywords: [],
      });

      expect(result).toBe(false);
      expect(store.getState().live.sessionId).toBe("session-a");
    });
  });
});
