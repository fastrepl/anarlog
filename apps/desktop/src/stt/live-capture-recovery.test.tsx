import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCaptureSnapshot: vi.fn(),
  listCaptureRecoveries: vi.fn(),
  listenCaptureRecoveryRequests: vi.fn(),
  recoveryRequestHandler: undefined as
    | ((sessionId: string) => void)
    | undefined,
  resumeHook: vi.fn(),
  resumeBySession: new Map<string, ReturnType<typeof vi.fn>>(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    getCaptureSnapshot: mocks.getCaptureSnapshot,
    listCaptureRecoveries: mocks.listCaptureRecoveries,
  },
}));

vi.mock("./useStartListening", () => ({
  useResumeListeningLifecycle: (sessionId: string) => {
    mocks.resumeHook(sessionId);
    let resume = mocks.resumeBySession.get(sessionId);
    if (!resume) {
      resume = vi.fn(() => Promise.resolve("attached"));
      mocks.resumeBySession.set(sessionId, resume);
    }
    return resume;
  },
}));

vi.mock("./capture-recovery-requests", () => ({
  listenCaptureRecoveryRequests: mocks.listenCaptureRecoveryRequests,
}));

vi.mock("./capture-lifecycle-storage", () => ({
  loadCaptureLifecycleMarker: vi.fn(),
  hasPendingZeroRetentionAudio: vi.fn(),
}));

import { LiveCaptureRecovery } from "./live-capture-recovery";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.resumeBySession.clear();
  mocks.recoveryRequestHandler = undefined;
  mocks.getCaptureSnapshot.mockResolvedValue({
    status: "ok",
    data: {
      activeSessionId: null,
      finalizingSessionIds: [],
      liveTranscriptionActive: null,
      requestedLiveTranscription: null,
      state: "inactive",
    },
  });
  mocks.listCaptureRecoveries.mockResolvedValue({ status: "ok", data: [] });
  mocks.listenCaptureRecoveryRequests.mockImplementation(
    async (handler: (sessionId: string) => void) => {
      mocks.recoveryRequestHandler = handler;
      return vi.fn();
    },
  );
});

test("restores lifecycle callbacks for native captures after renderer reload", async () => {
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [
      { session_id: "session-active", process_stopped: false },
      { session_id: "session-finalizing", process_stopped: false },
    ],
  });

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(mocks.resumeBySession.get("session-active")).toHaveBeenCalledOnce();
    expect(
      mocks.resumeBySession.get("session-finalizing"),
    ).toHaveBeenCalledOnce();
  });
});

test("does not install lifecycle callbacks without a native capture", async () => {
  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(mocks.listCaptureRecoveries).toHaveBeenCalledOnce();
  });
  expect(mocks.resumeBySession.size).toBe(0);
});

test("finalizes a durable capture marker after a stop event was missed", async () => {
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [
      {
        session_id: "session-missed-stop",
        process_stopped: true,
      },
    ],
  });

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    const resume = mocks.resumeBySession.get("session-missed-stop");
    expect(resume).toHaveBeenCalledOnce();
    expect(resume).toHaveBeenCalledWith(
      expect.objectContaining({ processStopped: true }),
    );
  });
});

test("retries normal post-stop recovery requests in the main window", async () => {
  render(<LiveCaptureRecovery />);
  await waitFor(() => {
    expect(mocks.recoveryRequestHandler).toBeTypeOf("function");
  });

  act(() => {
    mocks.recoveryRequestHandler?.("session-post-stop");
  });

  await waitFor(() => {
    expect(
      mocks.resumeBySession.get("session-post-stop"),
    ).toHaveBeenCalledOnce();
  });
});

test("remounts recovery for a later capture on the same session", async () => {
  render(<LiveCaptureRecovery />);
  await waitFor(() => {
    expect(mocks.recoveryRequestHandler).toBeTypeOf("function");
  });

  act(() => {
    mocks.recoveryRequestHandler?.("session-reused");
  });
  await waitFor(() => {
    expect(mocks.resumeBySession.get("session-reused")).toHaveBeenCalledOnce();
  });

  act(() => {
    mocks.recoveryRequestHandler?.("session-reused");
  });
  await waitFor(() => {
    expect(mocks.resumeBySession.get("session-reused")).toHaveBeenCalledTimes(
      2,
    );
  });
  expect(mocks.resumeHook).toHaveBeenCalledTimes(2);
});

test("retries recovery for captures returned by the native aggregate command", async () => {
  vi.useFakeTimers();
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [{ session_id: "session-retry", process_stopped: false }],
  });
  const resume = vi
    .fn()
    .mockRejectedValueOnce(new Error("database is locked"))
    .mockResolvedValueOnce("attached");
  mocks.resumeBySession.set("session-retry", resume);

  render(<LiveCaptureRecovery />);
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(resume).toHaveBeenCalledOnce();

  await act(async () => {
    vi.advanceTimersByTime(2_000);
    await Promise.resolve();
  });
  expect(resume).toHaveBeenCalledTimes(2);
  consoleError.mockRestore();
  vi.useRealTimers();
});

test("stops automatic recovery after the bounded retry budget", async () => {
  vi.useFakeTimers();
  const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [{ session_id: "session-terminal", process_stopped: false }],
  });
  const resume = vi.fn().mockResolvedValue("error");
  mocks.resumeBySession.set("session-terminal", resume);

  render(<LiveCaptureRecovery />);
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  expect(resume).toHaveBeenCalledTimes(5);
  expect(resume).toHaveBeenNthCalledWith(5, {
    abandonOnFailure: true,
    processStopped: false,
  });

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  expect(resume).toHaveBeenCalledTimes(5);
  expect(consoleWarn).toHaveBeenCalledWith(
    "[listener] capture recovery retry budget exhausted",
    { sessionId: "session-terminal" },
  );
  consoleWarn.mockRestore();
  vi.useRealTimers();
});

test("recovers durable markers returned by the native aggregate command", async () => {
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [{ session_id: "session-marker", process_stopped: false }],
  });

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(mocks.resumeBySession.get("session-marker")).toHaveBeenCalledOnce();
  });
});

test("leaves captures found after a reload for the user to process", async () => {
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [{ session_id: "session-saved", process_stopped: false }],
  });
  const resume = vi.fn().mockResolvedValue("awaiting_user");
  mocks.resumeBySession.set("session-saved", resume);

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(resume).toHaveBeenCalledWith({
      abandonOnFailure: false,
      processStopped: false,
    });
  });
  await waitFor(() => expect(mocks.recoveryRequestHandler).toBeDefined());

  act(() => {
    mocks.recoveryRequestHandler?.("session-saved");
  });

  await waitFor(() => {
    expect(resume).toHaveBeenLastCalledWith({
      abandonOnFailure: false,
      processStopped: true,
    });
  });
});

test("recovers a capture that could not be reattached once it stops", async () => {
  vi.useFakeTimers();
  const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
  const snapshot = (activeSessionId: string | null) => ({
    status: "ok",
    data: {
      activeSessionId,
      finalizingSessionIds: [],
      liveTranscriptionActive: null,
      requestedLiveTranscription: null,
      state: activeSessionId ? "active" : "inactive",
    },
  });
  mocks.listCaptureRecoveries.mockResolvedValue({
    status: "ok",
    data: [{ session_id: "session-live", process_stopped: false }],
  });
  mocks.getCaptureSnapshot.mockResolvedValue(snapshot("session-live"));
  const resume = vi.fn().mockResolvedValue("error");
  mocks.resumeBySession.set("session-live", resume);

  render(<LiveCaptureRecovery />);
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  expect(resume).toHaveBeenCalledTimes(5);

  resume.mockResolvedValue("awaiting_user");
  mocks.getCaptureSnapshot.mockResolvedValue(snapshot(null));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(5_000);
  });
  expect(resume).toHaveBeenCalledTimes(6);
  expect(resume).toHaveBeenLastCalledWith({
    abandonOnFailure: true,
    processStopped: false,
  });

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  expect(resume).toHaveBeenCalledTimes(6);
  consoleWarn.mockRestore();
  vi.useRealTimers();
});
