import { beforeEach, describe, expect, test, vi } from "vitest";

import { runBatchSession } from "./general-batch";

const { listenMock, startTranscriptionMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  startTranscriptionMock: vi.fn(),
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
