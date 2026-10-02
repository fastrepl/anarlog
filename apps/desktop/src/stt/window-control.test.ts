import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  emitTo: vi.fn(async () => {}),
  stopCaptureForSession: vi.fn(async () => ({ status: "ok", data: true })),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emitTo: hoisted.emitTo,
  listen: vi.fn(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    stopCaptureForSession: hoisted.stopCaptureForSession,
  },
}));

vi.mock("@anlg/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: () => "note-1",
}));

vi.mock("./contexts", () => ({ useListener: vi.fn() }));
vi.mock("./useStartListeningWithBatchOverride", () => ({
  useStartListeningWithBatchOverride: vi.fn(),
}));
vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: { getState: vi.fn() },
}));

import { requestMainListenerControl } from "./window-control";

describe("requestMainListenerControl", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  beforeEach(() => {
    vi.useFakeTimers();
    hoisted.emitTo.mockClear();
    hoisted.stopCaptureForSession.mockClear();
  });

  test("asks the main webview first, then stops that session natively", async () => {
    const request = requestMainListenerControl("stop", "session-1");

    expect(hoisted.emitTo).toHaveBeenCalledWith(
      "main",
      "anlg:listener-control",
      expect.objectContaining({ action: "stop", sessionId: "session-1" }),
    );
    expect(hoisted.stopCaptureForSession).not.toHaveBeenCalled();

    await vi.runAllTimersAsync();
    await request;

    expect(hoisted.stopCaptureForSession).toHaveBeenCalledTimes(1);
    expect(hoisted.stopCaptureForSession).toHaveBeenCalledWith("session-1");
  });

  test("stops natively when the main window cannot be reached", async () => {
    hoisted.emitTo.mockRejectedValueOnce(new Error("window not found"));

    const request = requestMainListenerControl("stop", "session-1");
    await vi.runAllTimersAsync();
    await request;

    expect(hoisted.stopCaptureForSession).toHaveBeenCalledWith("session-1");
  });

  test("still routes start requests to the main webview", async () => {
    await requestMainListenerControl("start", "session-1");

    expect(hoisted.emitTo).toHaveBeenCalledWith(
      "main",
      "anlg:listener-control",
      expect.objectContaining({ action: "start", sessionId: "session-1" }),
    );
    expect(hoisted.stopCaptureForSession).not.toHaveBeenCalled();
  });
});
