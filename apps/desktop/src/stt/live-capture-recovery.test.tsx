import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCaptureSnapshot: vi.fn(),
  listenCaptureRecoveryRequests: vi.fn(),
  loadCaptureLifecycleMarkers: vi.fn(),
  recoveryRequestHandler: undefined as
    | ((sessionId: string) => void)
    | undefined,
  resumeHook: vi.fn(),
  resumeBySession: new Map<string, ReturnType<typeof vi.fn>>(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    getCaptureSnapshot: mocks.getCaptureSnapshot,
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
  loadCaptureLifecycleMarkers: mocks.loadCaptureLifecycleMarkers,
}));

import { LiveCaptureRecovery } from "./live-capture-recovery";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.resumeBySession.clear();
  mocks.recoveryRequestHandler = undefined;
  mocks.listenCaptureRecoveryRequests.mockImplementation(
    async (handler: (sessionId: string) => void) => {
      mocks.recoveryRequestHandler = handler;
      return vi.fn();
    },
  );
  mocks.loadCaptureLifecycleMarkers.mockResolvedValue([]);
});

test("restores lifecycle callbacks for native captures after renderer reload", async () => {
  mocks.getCaptureSnapshot.mockResolvedValue({
    status: "ok",
    data: {
      activeSessionId: "session-active",
      finalizingSessionIds: ["session-finalizing"],
      liveTranscriptionActive: true,
      requestedLiveTranscription: true,
      state: "active",
    },
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

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(mocks.getCaptureSnapshot).toHaveBeenCalledOnce();
  });
  expect(mocks.resumeBySession.size).toBe(0);
});

test("finalizes a durable capture marker after a stop event was missed", async () => {
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
  mocks.loadCaptureLifecycleMarkers.mockResolvedValue([
    { sessionId: "session-missed-stop" },
  ]);

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(
      mocks.resumeBySession.get("session-missed-stop"),
    ).toHaveBeenCalledOnce();
  });
});

test("retries normal post-stop recovery requests in the main window", async () => {
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

test("retries durable capture recovery when the native snapshot is unavailable", async () => {
  vi.useFakeTimers();
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.getCaptureSnapshot.mockResolvedValue({
    status: "error",
    error: "capture snapshot unavailable",
  });
  mocks.loadCaptureLifecycleMarkers.mockResolvedValue([
    { sessionId: "session-retry" },
  ]);
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
  mocks.loadCaptureLifecycleMarkers.mockResolvedValue([
    { sessionId: "session-terminal" },
  ]);
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

test("recovers durable markers when the native snapshot command rejects", async () => {
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.getCaptureSnapshot.mockRejectedValue(
    new Error("capture actor unavailable"),
  );
  mocks.loadCaptureLifecycleMarkers.mockResolvedValue([
    { sessionId: "session-marker" },
  ]);

  render(<LiveCaptureRecovery />);

  await waitFor(() => {
    expect(mocks.resumeBySession.get("session-marker")).toHaveBeenCalledOnce();
  });
  consoleError.mockRestore();
});
