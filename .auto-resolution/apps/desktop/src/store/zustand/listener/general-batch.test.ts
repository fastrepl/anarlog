import { beforeEach, describe, expect, test, vi } from "vitest";

import { EMPTY_BATCH_TRANSCRIPT_ERROR } from "./batch";
import {
  runBatchSession,
  showBatchCompletedNotification,
  shouldUseSyntheticBatchProgress,
  syntheticBatchProgress,
} from "./general-batch";

import { parseBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";

const {
  isFocusedMock,
  isVisibleMock,
  listenMock,
  showNotificationMock,
  startTranscriptionMock,
} = vi.hoisted(() => ({
  isFocusedMock: vi.fn(),
  isVisibleMock: vi.fn(),
  listenMock: vi.fn(),
  showNotificationMock: vi.fn(),
  startTranscriptionMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isFocused: isFocusedMock,
    isVisible: isVisibleMock,
  }),
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: {
    showNotification: showNotificationMock,
  },
}));

vi.mock("@hypr/plugin-transcription", () => ({
  events: {
    transcriptionEvent: {
      listen: listenMock,
    },
  },
  commands: {
    startTranscription: startTranscriptionMock,
  },
}));

describe("runBatchSession", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isFocusedMock.mockResolvedValue(true);
    isVisibleMock.mockResolvedValue(true);
    showNotificationMock.mockResolvedValue({ status: "ok", data: null });
  });

  test("uses synthetic progress only for blocking batch providers", () => {
    expect(
      shouldUseSyntheticBatchProgress({
        session_id: "session-1",
        provider: "hyprnote",
        file_path: "/tmp/session.wav",
        base_url: "",
        api_key: "",
      }),
    ).toBe(true);
    expect(
      shouldUseSyntheticBatchProgress({
        session_id: "session-1",
        provider: "soniqo",
        file_path: "/tmp/session.wav",
        base_url: "soniqo://local",
        api_key: "",
      }),
    ).toBe(false);
    expect(
      shouldUseSyntheticBatchProgress({
        session_id: "session-1",
        provider: "openai",
        file_path: "/tmp/session.wav",
        model: "gpt-4o-transcribe",
        base_url: "",
        api_key: "",
      }),
    ).toBe(false);
    expect(
      shouldUseSyntheticBatchProgress({
        session_id: "session-1",
        provider: "am",
        file_path: "/tmp/session.wav",
        base_url: "https://api.deepgram.com/v1",
        api_key: "",
      }),
    ).toBe(true);
    expect(
      shouldUseSyntheticBatchProgress({
        session_id: "session-1",
        provider: "am",
        file_path: "/tmp/session.wav",
        base_url: "http://localhost:50060/v1",
        api_key: "",
      }),
    ).toBe(false);
    expect(
      shouldUseSyntheticBatchProgress({
        session_id: "session-1",
        provider: "am",
        file_path: "/tmp/session.wav",
        model: "gpt-4o-transcribe",
        base_url: "https://api.openai.com/v1",
        api_key: "",
      }),
    ).toBe(false);
  });

  test("caps synthetic progress before completion", () => {
    expect(syntheticBatchProgress(0)).toBe(0.06);
    expect(syntheticBatchProgress(60_000)).toBeLessThan(0.88);
    expect(syntheticBatchProgress(10_000_000)).toBe(0.88);
  });

  test("ticks synthetic progress for blocking batch providers", async () => {
    vi.useFakeTimers();

    try {
      const handleBatchStarted = vi.fn();
      const handleBatchResponse = vi.fn();
      const handleBatchCompleted = vi.fn();
      const clearBatchPersist = vi.fn();
      const clearBatchSession = vi.fn();
      const handleBatchResponseStreamed = vi.fn();
      const handleBatchFailed = vi.fn();
      const handleBatchStopped = vi.fn();
      const updateBatchProgress = vi.fn();
      const setBatchPersist = vi.fn();

      let handler:
        | ((event: {
            payload: {
              type: string;
              session_id: string;
              response?: unknown;
              mode?: "direct" | "streamed";
            };
          }) => void)
        | undefined;
      let resolveStart: ((value: unknown) => void) | undefined;

      listenMock.mockImplementation(async (cb) => {
        handler = cb;
        return vi.fn();
      });

      startTranscriptionMock.mockImplementation(
        () =>
          new Promise((resolve) => {
            resolveStart = resolve;
          }),
      );

      const runPromise = runBatchSession(
        () => ({
          batch: {},
          batchPreview: {},
          batchPersist: {},
          handleBatchStarted,
          handleBatchResponse,
          handleBatchCompleted,
          clearBatchPersist,
          clearBatchSession,
          handleBatchResponseStreamed,
          handleBatchFailed,
          handleBatchStopped,
          updateBatchProgress,
          setBatchPersist,
        }),
        "session-1",
        {
          session_id: "session-1",
          provider: "hyprnote",
          file_path: "/tmp/session.wav",
          base_url: "",
          api_key: "",
        },
      );

      await Promise.resolve();
      await Promise.resolve();

      expect(updateBatchProgress).toHaveBeenCalledWith("session-1", 0.06);

      vi.advanceTimersByTime(1_600);

      expect(
        updateBatchProgress.mock.calls.some(
          ([, percentage]) => percentage > 0.06 && percentage < 0.88,
        ),
      ).toBe(true);

      handler?.({
        payload: {
          type: "completed",
          session_id: "session-1",
          mode: "direct",
          response: {
            metadata: null,
            results: { channels: [] },
          },
        },
      });
      resolveStart?.({ status: "ok", data: null });

      await runPromise;

      const callCount = updateBatchProgress.mock.calls.length;
      vi.advanceTimersByTime(1_600);
      expect(updateBatchProgress).toHaveBeenCalledTimes(callCount);
    } finally {
      vi.useRealTimers();
    }
  });

  test("keeps synthetic progress when the backend emits started", async () => {
    vi.useFakeTimers();

    try {
      const handleBatchStarted = vi.fn();
      const handleBatchResponse = vi.fn();
      const handleBatchCompleted = vi.fn();
      const clearBatchPersist = vi.fn();
      const clearBatchSession = vi.fn();
      const handleBatchResponseStreamed = vi.fn();
      const handleBatchFailed = vi.fn();
      const handleBatchStopped = vi.fn();
      const updateBatchProgress = vi.fn();
      const setBatchPersist = vi.fn();

      let handler:
        | ((event: {
            payload: {
              type: string;
              session_id: string;
              response?: unknown;
              mode?: "direct" | "streamed";
            };
          }) => void)
        | undefined;
      let resolveStart: ((value: unknown) => void) | undefined;

      listenMock.mockImplementation(async (cb) => {
        handler = cb;
        return vi.fn();
      });

      startTranscriptionMock.mockImplementation(
        () =>
          new Promise((resolve) => {
            resolveStart = resolve;
          }),
      );

      const runPromise = runBatchSession(
        () => ({
          batch: {},
          batchPreview: {},
          batchPersist: {},
          handleBatchStarted,
          handleBatchResponse,
          handleBatchCompleted,
          clearBatchPersist,
          clearBatchSession,
          handleBatchResponseStreamed,
          handleBatchFailed,
          handleBatchStopped,
          updateBatchProgress,
          setBatchPersist,
        }),
        "session-1",
        {
          session_id: "session-1",
          provider: "hyprnote",
          file_path: "/tmp/session.wav",
          base_url: "",
          api_key: "",
        },
      );

      await Promise.resolve();
      await Promise.resolve();

      handler?.({
        payload: {
          type: "started",
          session_id: "session-1",
        },
      });

      expect(handleBatchStarted).toHaveBeenCalledTimes(1);
      expect(updateBatchProgress).toHaveBeenCalledWith("session-1", 0.06);

      handler?.({
        payload: {
          type: "completed",
          session_id: "session-1",
          mode: "direct",
          response: {
            metadata: null,
            results: { channels: [] },
          },
        },
      });
      resolveStart?.({ status: "ok", data: null });

      await runPromise;
    } finally {
      vi.useRealTimers();
    }
  });

  test("resolves from the completed event and persists the response", async () => {
    const handleBatchStarted = vi.fn();
    const handleBatchResponse = vi.fn();
    const handleBatchCompleted = vi.fn();
    const clearBatchPersist = vi.fn();
    const clearBatchSession = vi.fn();
    const handleBatchResponseStreamed = vi.fn();
    const handleBatchFailed = vi.fn();
    const handleBatchStopped = vi.fn();
    const updateBatchProgress = vi.fn();
    const setBatchPersist = vi.fn();

    let handler:
      | ((event: {
          payload: {
            type: string;
            session_id: string;
            response?: unknown;
            mode?: "direct" | "streamed";
          };
        }) => void)
      | undefined;

    listenMock.mockImplementation(async (cb) => {
      handler = cb;
      return vi.fn();
    });

    startTranscriptionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          queueMicrotask(() => {
            handler?.({
              payload: {
                type: "completed",
                session_id: "session-1",
                mode: "streamed",
                response: {
                  metadata: null,
                  results: { channels: [] },
                },
              },
            });
            resolve({
              status: "ok",
              data: null,
            });
          });
        }),
    );

    await runBatchSession(
      () => ({
        batch: {},
        batchPreview: {},
        batchPersist: {},
        handleBatchStarted,
        handleBatchResponse,
        handleBatchCompleted,
        clearBatchPersist,
        clearBatchSession,
        handleBatchResponseStreamed,
        handleBatchFailed,
        handleBatchStopped,
        updateBatchProgress,
        setBatchPersist,
      }),
      "session-1",
      {
        session_id: "session-1",
        provider: "hyprnote",
        file_path: "/tmp/session.wav",
        base_url: "",
        api_key: "",
      },
    );

    expect(handleBatchStarted).toHaveBeenCalledWith("session-1");
    expect(handleBatchResponse).toHaveBeenCalledWith("session-1", {
      metadata: null,
      results: { channels: [] },
    });
    expect(clearBatchPersist).toHaveBeenCalledWith("session-1");
    expect(clearBatchSession).toHaveBeenCalledWith("session-1");
    expect(handleBatchFailed).not.toHaveBeenCalled();
    expect(handleBatchResponseStreamed).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("shows a completion notification when the window is not focused", async () => {
    isFocusedMock.mockResolvedValue(false);

    const handleBatchStarted = vi.fn();
    const handleBatchResponse = vi.fn();
    const handleBatchCompleted = vi.fn();
    const clearBatchPersist = vi.fn();
    const clearBatchSession = vi.fn();
    const handleBatchResponseStreamed = vi.fn();
    const handleBatchFailed = vi.fn();
    const handleBatchStopped = vi.fn();
    const updateBatchProgress = vi.fn();
    const setBatchPersist = vi.fn();

    let handler:
      | ((event: {
          payload: {
            type: string;
            session_id: string;
            response?: unknown;
            mode?: "direct" | "streamed";
          };
        }) => void)
      | undefined;

    listenMock.mockImplementation(async (cb) => {
      handler = cb;
      return vi.fn();
    });

    startTranscriptionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          queueMicrotask(() => {
            handler?.({
              payload: {
                type: "completed",
                session_id: "session-1",
                mode: "streamed",
                response: {
                  metadata: null,
                  results: { channels: [] },
                },
              },
            });
            resolve({
              status: "ok",
              data: null,
            });
          });
        }),
    );

    await runBatchSession(
      () => ({
        batch: {},
        batchPreview: {},
        batchPersist: {},
        handleBatchStarted,
        handleBatchResponse,
        handleBatchCompleted,
        clearBatchPersist,
        clearBatchSession,
        handleBatchResponseStreamed,
        handleBatchFailed,
        handleBatchStopped,
        updateBatchProgress,
        setBatchPersist,
      }),
      "session-1",
      {
        session_id: "session-1",
        provider: "hyprnote",
        file_path: "/tmp/session.wav",
        base_url: "",
        api_key: "",
      },
    );

    const notification = showNotificationMock.mock.calls[0]?.[0];
    expect(notification).toEqual(
      expect.objectContaining({
        title: "Transcription complete",
        message: "Your transcript is ready.",
        action_label: "Open Anarlog",
        source: { type: "session", session_id: "session-1" },
      }),
    );
    expect(parseBatchCompletedNotificationKey(notification.key)).toBe(
      "session-1",
    );
  });

  test("uses a fresh notification key for each batch completion", async () => {
    await showBatchCompletedNotification("session-1", { force: true });
    await showBatchCompletedNotification("session-1", { force: true });

    const firstKey = showNotificationMock.mock.calls[0]?.[0].key;
    const secondKey = showNotificationMock.mock.calls[1]?.[0].key;

    expect(parseBatchCompletedNotificationKey(firstKey)).toBe("session-1");
    expect(parseBatchCompletedNotificationKey(secondKey)).toBe("session-1");
    expect(firstKey).not.toBe(secondKey);
  });

  test("forwards streamed progress events before completion", async () => {
    const handleBatchStarted = vi.fn();
    const handleBatchResponse = vi.fn();
    const handleBatchCompleted = vi.fn();
    const clearBatchPersist = vi.fn();
    const clearBatchSession = vi.fn();
    const handleBatchResponseStreamed = vi.fn();
    const handleBatchFailed = vi.fn();
    const handleBatchStopped = vi.fn();
    const updateBatchProgress = vi.fn();
    const setBatchPersist = vi.fn();

    let handler:
      | ((event: {
          payload: {
            type: string;
            session_id: string;
            event?: unknown;
            response?: unknown;
            mode?: "direct" | "streamed";
          };
        }) => void)
      | undefined;

    listenMock.mockImplementation(async (cb) => {
      handler = cb;
      return vi.fn();
    });

    const progressEvent = {
      type: "progress" as const,
      percentage: 0.42,
      partial_text: "hello there",
    };

    startTranscriptionMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          queueMicrotask(() => {
            handler?.({
              payload: {
                type: "progress",
                session_id: "session-1",
                event: progressEvent,
              },
            });
            handler?.({
              payload: {
                type: "completed",
                session_id: "session-1",
                mode: "streamed",
                response: {
                  metadata: null,
                  results: { channels: [] },
                },
              },
            });
            resolve({
              status: "ok",
              data: null,
            });
          });
        }),
    );

    await runBatchSession(
      () => ({
        batch: {},
        batchPreview: {},
        batchPersist: {},
        handleBatchStarted,
        handleBatchResponse,
        handleBatchCompleted,
        clearBatchPersist,
        clearBatchSession,
        handleBatchResponseStreamed,
        handleBatchFailed,
        handleBatchStopped,
        updateBatchProgress,
        setBatchPersist,
      }),
      "session-1",
      {
        session_id: "session-1",
        provider: "hyprnote",
        file_path: "/tmp/session.wav",
        base_url: "",
        api_key: "",
      },
    );

    expect(handleBatchResponseStreamed).toHaveBeenCalledWith(
      "session-1",
      progressEvent,
    );
    expect(handleBatchResponse).toHaveBeenCalledWith("session-1", {
      metadata: null,
      results: { channels: [] },
    });
    expect(handleBatchFailed).not.toHaveBeenCalled();
  });

  test("rejects completed responses that have no transcribed words", async () => {
    const handleBatchStarted = vi.fn();
    const handleBatchResponse = vi.fn(() => false);
    const handleBatchCompleted = vi.fn();
    const clearBatchPersist = vi.fn();
    const clearBatchSession = vi.fn();
    const handleBatchResponseStreamed = vi.fn();
    const handleBatchFailed = vi.fn();
    const handleBatchStopped = vi.fn();
    const updateBatchProgress = vi.fn();
    const setBatchPersist = vi.fn();

    let handler:
      | ((event: {
          payload: {
            type: string;
            session_id: string;
            response?: unknown;
            mode?: "direct" | "streamed";
          };
        }) => void)
      | undefined;

    listenMock.mockImplementation(async (cb) => {
      handler = cb;
      return vi.fn();
    });

    startTranscriptionMock.mockImplementation(async () => {
      queueMicrotask(() => {
        handler?.({
          payload: {
            type: "completed",
            session_id: "session-1",
            mode: "direct",
            response: {
              metadata: null,
              results: { channels: [] },
            },
          },
        });
      });

      return {
        status: "ok",
        data: null,
      };
    });

    await expect(
      runBatchSession(
        () => ({
          batch: {},
          batchPreview: {},
          batchPersist: {},
          handleBatchStarted,
          handleBatchResponse,
          handleBatchCompleted,
          clearBatchPersist,
          clearBatchSession,
          handleBatchResponseStreamed,
          handleBatchFailed,
          handleBatchStopped,
          updateBatchProgress,
          setBatchPersist,
        }),
        "session-1",
        {
          session_id: "session-1",
          provider: "hyprnote",
          file_path: "/tmp/session.wav",
          base_url: "",
          api_key: "",
        },
      ),
    ).rejects.toThrow(EMPTY_BATCH_TRANSCRIPT_ERROR);

    expect(handleBatchFailed).toHaveBeenCalledWith(
      "session-1",
      EMPTY_BATCH_TRANSCRIPT_ERROR,
    );
    expect(clearBatchPersist).toHaveBeenCalledWith("session-1");
    expect(clearBatchSession).not.toHaveBeenCalled();
  });

  test("rejects when the transcription is stopped", async () => {
    const handleBatchStarted = vi.fn();
    const handleBatchResponse = vi.fn();
    const handleBatchCompleted = vi.fn();
    const clearBatchPersist = vi.fn();
    const clearBatchSession = vi.fn();
    const handleBatchResponseStreamed = vi.fn();
    const handleBatchFailed = vi.fn();
    const handleBatchStopped = vi.fn();
    const updateBatchProgress = vi.fn();
    const setBatchPersist = vi.fn();

    let handler:
      | ((event: {
          payload:
            | { type: "started"; session_id: string }
            | { type: "stopped"; session_id: string };
        }) => void)
      | undefined;

    listenMock.mockImplementation(async (cb) => {
      handler = cb;
      return vi.fn();
    });

    startTranscriptionMock.mockImplementation(async () => {
      queueMicrotask(() => {
        handler?.({
          payload: {
            type: "stopped",
            session_id: "session-1",
          },
        });
      });

      return { status: "ok", data: null };
    });

    await expect(
      runBatchSession(
        () => ({
          batch: {},
          batchPreview: {},
          batchPersist: {},
          handleBatchStarted,
          handleBatchResponse,
          handleBatchCompleted,
          clearBatchPersist,
          clearBatchSession,
          handleBatchResponseStreamed,
          handleBatchFailed,
          handleBatchStopped,
          updateBatchProgress,
          setBatchPersist,
        }),
        "session-1",
        {
          session_id: "session-1",
          provider: "hyprnote",
          file_path: "/tmp/session.wav",
          base_url: "",
          api_key: "",
        },
      ),
    ).rejects.toThrow("Transcription stopped.");

    expect(handleBatchStopped).toHaveBeenCalledWith("session-1");
    expect(handleBatchFailed).not.toHaveBeenCalled();
    expect(clearBatchSession).not.toHaveBeenCalled();
  });

  test("marks timed out failures distinctly", async () => {
    const handleBatchStarted = vi.fn();
    const handleBatchResponse = vi.fn();
    const handleBatchCompleted = vi.fn();
    const clearBatchPersist = vi.fn();
    const clearBatchSession = vi.fn();
    const handleBatchResponseStreamed = vi.fn();
    const handleBatchFailed = vi.fn();
    const handleBatchStopped = vi.fn();
    const updateBatchProgress = vi.fn();
    const setBatchPersist = vi.fn();

    let handler:
      | ((event: {
          payload:
            | {
                type: "failed";
                session_id: string;
                code: "timed_out";
                error: string;
              }
            | { type: "started"; session_id: string };
        }) => void)
      | undefined;

    listenMock.mockImplementation(async (cb) => {
      handler = cb;
      return vi.fn();
    });

    startTranscriptionMock.mockImplementation(async () => {
      queueMicrotask(() => {
        handler?.({
          payload: {
            type: "failed",
            session_id: "session-1",
            code: "timed_out",
            error: "Transcription timed out after 60 seconds without progress.",
          },
        });
      });

      return { status: "ok", data: null };
    });

    await expect(
      runBatchSession(
        () => ({
          batch: {},
          batchPreview: {},
          batchPersist: {},
          handleBatchStarted,
          handleBatchResponse,
          handleBatchCompleted,
          clearBatchPersist,
          clearBatchSession,
          handleBatchResponseStreamed,
          handleBatchFailed,
          handleBatchStopped,
          updateBatchProgress,
          setBatchPersist,
        }),
        "session-1",
        {
          session_id: "session-1",
          provider: "hyprnote",
          file_path: "/tmp/session.wav",
          base_url: "",
          api_key: "",
        },
      ),
    ).rejects.toBe(
      "Transcription timed out after 60 seconds without progress.",
    );

    expect(handleBatchFailed).toHaveBeenCalledWith(
      "session-1",
      "Transcription timed out after 60 seconds without progress.",
      "timed_out",
      "timed_out",
    );
    expect(handleBatchStopped).not.toHaveBeenCalled();
  });
});
