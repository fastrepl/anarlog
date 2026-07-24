import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  callbacks: new Map<string, (event: { payload: unknown }) => void>(),
  emit: vi.fn().mockResolvedValue(undefined),
  emitTo: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn(),
  currentLabel: "note-window",
  id: vi.fn(() => "request-id"),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
  emitTo: mocks.emitTo,
  listen: mocks.listen,
}));

vi.mock("@hypr/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: () => mocks.currentLabel,
}));

vi.mock("~/shared/utils", () => ({
  id: mocks.id,
}));

import {
  MAIN_AUTO_ENHANCE_TIMEOUT_MS,
  handleMainAutoEnhanceRequest,
  handleMainEnhanceRequest,
  requestMainAutoEnhance,
} from "./task-window-sync";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.callbacks.clear();
  mocks.currentLabel = "note-window";
  mocks.id.mockReturnValue("request-id");
  mocks.emitTo.mockResolvedValue(undefined);
  mocks.listen.mockImplementation(async (eventName, callback) => {
    mocks.callbacks.set(eventName, callback);
    return () => {
      if (mocks.callbacks.get(eventName) === callback) {
        mocks.callbacks.delete(eventName);
      }
    };
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("handleMainEnhanceRequest", () => {
  it.each(["if_empty", "regenerate"] as const)(
    "routes %s through the retry-aware auto-enhance entrypoint",
    async (mode) => {
      const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);
      const enhance = vi.fn().mockResolvedValue({ type: "no_model" });

      await handleMainEnhanceRequest(
        {
          requestAutoEnhance,
          enhance,
        },
        {
          sessionId: "session-1",
          auto: mode,
        },
      );

      expect(requestAutoEnhance).toHaveBeenCalledWith("session-1", mode);
      expect(enhance).not.toHaveBeenCalled();
    },
  );

  it("keeps explicit enhancement requests on the direct path", async () => {
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);
    const enhance = vi.fn().mockResolvedValue({ type: "no_model" });
    const opts = { isAuto: false, templateId: "template-1" };

    await handleMainEnhanceRequest(
      {
        requestAutoEnhance,
        enhance,
      },
      {
        sessionId: "session-1",
        opts,
      },
    );

    expect(enhance).toHaveBeenCalledWith("session-1", opts);
    expect(requestAutoEnhance).not.toHaveBeenCalled();
  });
});

describe("requestMainAutoEnhance", () => {
  it("waits for the main window to durably handle the request", async () => {
    const request = requestMainAutoEnhance("session-1", "regenerate");

    await vi.waitFor(() =>
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "hypr:ai-task-auto-enhance-request",
        {
          requestId: "request-id",
          sourceLabel: "note-window",
          sessionId: "session-1",
          mode: "regenerate",
        },
      ),
    );

    mocks.callbacks.get("hypr:ai-task-auto-enhance-result")?.({
      payload: {
        requestId: "request-id",
        completed: true,
        error: null,
      },
    });

    await expect(request).resolves.toBeUndefined();
    expect(mocks.callbacks.has("hypr:ai-task-auto-enhance-result")).toBe(false);
  });

  it("rejects a main-window scheduling error", async () => {
    const request = requestMainAutoEnhance("session-1", "if_empty");
    await vi.waitFor(() =>
      expect(mocks.callbacks.has("hypr:ai-task-auto-enhance-result")).toBe(
        true,
      ),
    );

    mocks.callbacks.get("hypr:ai-task-auto-enhance-result")?.({
      payload: {
        requestId: "request-id",
        completed: false,
        error: "database is locked",
      },
    });

    await expect(request).rejects.toThrow("database is locked");
    expect(mocks.callbacks.has("hypr:ai-task-auto-enhance-result")).toBe(false);
  });

  it("rejects when the main window does not acknowledge in time", async () => {
    vi.useFakeTimers();
    const request = requestMainAutoEnhance("session-1", "regenerate");
    const rejection = expect(request).rejects.toThrow(
      "Main window did not acknowledge the auto-summary request in time",
    );

    await vi.advanceTimersByTimeAsync(MAIN_AUTO_ENHANCE_TIMEOUT_MS);
    await rejection;

    expect(mocks.callbacks.has("hypr:ai-task-auto-enhance-result")).toBe(false);
  });

  it("rejects when request dispatch fails", async () => {
    mocks.emitTo.mockRejectedValueOnce(new Error("event bus unavailable"));

    await expect(
      requestMainAutoEnhance("session-1", "regenerate"),
    ).rejects.toThrow("event bus unavailable");
    expect(mocks.callbacks.has("hypr:ai-task-auto-enhance-result")).toBe(false);
  });
});

describe("handleMainAutoEnhanceRequest", () => {
  it("acknowledges only after the durable service request completes", async () => {
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);

    await handleMainAutoEnhanceRequest(
      { requestAutoEnhance },
      {
        requestId: "request-id",
        sourceLabel: "note-window",
        sessionId: "session-1",
        mode: "regenerate",
      },
    );

    expect(requestAutoEnhance).toHaveBeenCalledWith("session-1", "regenerate");
    expect(requestAutoEnhance).toHaveBeenCalledBefore(mocks.emitTo);
    expect(mocks.emitTo).toHaveBeenCalledWith(
      "note-window",
      "hypr:ai-task-auto-enhance-result",
      {
        requestId: "request-id",
        completed: true,
        error: null,
      },
    );
    expect(mocks.emit).not.toHaveBeenCalled();
  });

  it("broadcasts the completed result when targeted acknowledgement fails", async () => {
    mocks.emitTo.mockRejectedValueOnce(new Error("target unavailable"));
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);

    await expect(
      handleMainAutoEnhanceRequest(
        { requestAutoEnhance },
        {
          requestId: "request-id",
          sourceLabel: "note-window",
          sessionId: "session-1",
          mode: "regenerate",
        },
      ),
    ).resolves.toBeUndefined();

    const result = {
      requestId: "request-id",
      completed: true,
      error: null,
    };
    expect(requestAutoEnhance).toHaveBeenCalledOnce();
    expect(mocks.emitTo).toHaveBeenCalledWith(
      "note-window",
      "hypr:ai-task-auto-enhance-result",
      result,
    );
    expect(mocks.emit).toHaveBeenCalledWith(
      "hypr:ai-task-auto-enhance-result",
      result,
    );
    expect(requestAutoEnhance).toHaveBeenCalledBefore(mocks.emitTo);
    expect(mocks.emitTo).toHaveBeenCalledBefore(mocks.emit);
  });

  it("does not repeat scheduling when both acknowledgement paths fail", async () => {
    mocks.emitTo.mockRejectedValueOnce(new Error("target unavailable"));
    mocks.emit.mockRejectedValueOnce(new Error("broadcast unavailable"));
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);

    await expect(
      handleMainAutoEnhanceRequest(
        { requestAutoEnhance },
        {
          requestId: "request-id",
          sourceLabel: "note-window",
          sessionId: "session-1",
          mode: "regenerate",
        },
      ),
    ).rejects.toThrow("broadcast unavailable");

    expect(requestAutoEnhance).toHaveBeenCalledOnce();
  });

  it("returns main-side errors to the requesting window", async () => {
    const requestAutoEnhance = vi
      .fn()
      .mockRejectedValue(new Error("summary marker write failed"));

    await handleMainAutoEnhanceRequest(
      { requestAutoEnhance },
      {
        requestId: "request-id",
        sourceLabel: "note-window",
        sessionId: "session-1",
        mode: "if_empty",
      },
    );

    expect(mocks.emitTo).toHaveBeenCalledWith(
      "note-window",
      "hypr:ai-task-auto-enhance-result",
      {
        requestId: "request-id",
        completed: false,
        error: "summary marker write failed",
      },
    );
  });

  it("reports when the main enhancer is not ready", async () => {
    await handleMainAutoEnhanceRequest(null, {
      requestId: "request-id",
      sourceLabel: "note-window",
      sessionId: "session-1",
      mode: "regenerate",
    });

    expect(mocks.emitTo).toHaveBeenCalledWith(
      "note-window",
      "hypr:ai-task-auto-enhance-result",
      {
        requestId: "request-id",
        completed: false,
        error: "Main auto-summary service is not ready",
      },
    );
  });
});
