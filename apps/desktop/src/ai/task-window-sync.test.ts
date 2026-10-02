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

vi.mock("@anlg/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: () => mocks.currentLabel,
}));

vi.mock("~/shared/utils", () => ({
  id: mocks.id,
}));

import {
  MAIN_AUTO_ENHANCE_TIMEOUT_MS,
  MAX_SYNCED_ENHANCE_TASKS,
  handleMainAutoEnhanceRequest,
  handleMainEnhanceRequest,
  requestMainAutoEnhance,
  serializeEnhanceTasks,
} from "./task-window-sync";

import { MAX_AI_TASK_STREAM_CHARACTERS } from "~/store/zustand/ai-task/tasks";

const autoEnhanceRequest = {
  requestId: "request-id",
  sourceLabel: "note-window",
  sessionId: "session-1",
  mode: "regenerate" as const,
};
const RESULT_EVENT = "anlg:ai-task-auto-enhance-result";

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
          sessionId: autoEnhanceRequest.sessionId,
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
        sessionId: autoEnhanceRequest.sessionId,
        opts,
      },
    );

    expect(enhance).toHaveBeenCalledWith("session-1", opts);
    expect(requestAutoEnhance).not.toHaveBeenCalled();
  });
});

describe("serializeEnhanceTasks", () => {
  it("serializes only a bounded recent enhance-task snapshot", () => {
    const tasks = Object.fromEntries([
      [
        "title-task",
        {
          taskType: "title",
          status: "success",
          streamedText: "Title",
          abortController: null,
        },
      ],
      ...Array.from({ length: MAX_SYNCED_ENHANCE_TASKS + 1 }, (_, index) => [
        `enhance-${index}`,
        {
          taskType: "enhance",
          status: "success",
          streamedText:
            index === MAX_SYNCED_ENHANCE_TASKS
              ? "x".repeat(MAX_AI_TASK_STREAM_CHARACTERS + 1)
              : `summary-${index}`,
          abortController: null,
        },
      ]),
    ]) as any;

    const serialized = serializeEnhanceTasks(tasks);

    expect(Object.keys(serialized)).toHaveLength(MAX_SYNCED_ENHANCE_TASKS);
    expect(serialized).not.toHaveProperty("title-task");
    expect(serialized).not.toHaveProperty("enhance-0");
    expect(
      serialized[`enhance-${MAX_SYNCED_ENHANCE_TASKS}`]?.streamedText,
    ).toHaveLength(MAX_AI_TASK_STREAM_CHARACTERS);
  });
});

describe("requestMainAutoEnhance", () => {
  it("waits for the main window to durably handle the request", async () => {
    const request = requestMainAutoEnhance(
      autoEnhanceRequest.sessionId,
      autoEnhanceRequest.mode,
    );

    await vi.waitFor(() =>
      expect(mocks.emitTo).toHaveBeenCalledWith(
        "main",
        "anlg:ai-task-auto-enhance-request",
        autoEnhanceRequest,
      ),
    );

    mocks.callbacks.get(RESULT_EVENT)?.({
      payload: {
        requestId: autoEnhanceRequest.requestId,
        completed: true,
        error: null,
      },
    });

    await expect(request).resolves.toBeUndefined();
    expect(mocks.callbacks.has(RESULT_EVENT)).toBe(false);
  });

  it("rejects a main-window scheduling error", async () => {
    const request = requestMainAutoEnhance(
      autoEnhanceRequest.sessionId,
      "if_empty",
    );
    await vi.waitFor(() =>
      expect(mocks.callbacks.has(RESULT_EVENT)).toBe(true),
    );

    mocks.callbacks.get(RESULT_EVENT)?.({
      payload: {
        requestId: autoEnhanceRequest.requestId,
        completed: false,
        error: "database is locked",
      },
    });

    await expect(request).rejects.toThrow("database is locked");
    expect(mocks.callbacks.has(RESULT_EVENT)).toBe(false);
  });

  it("rejects when the main window does not acknowledge in time", async () => {
    vi.useFakeTimers();
    const request = requestMainAutoEnhance(
      autoEnhanceRequest.sessionId,
      autoEnhanceRequest.mode,
    );
    const rejection = expect(request).rejects.toThrow(
      "Main window did not acknowledge the auto-summary request in time",
    );

    await vi.advanceTimersByTimeAsync(MAIN_AUTO_ENHANCE_TIMEOUT_MS);
    await rejection;

    expect(mocks.callbacks.has(RESULT_EVENT)).toBe(false);
  });

  it("rejects when request dispatch fails", async () => {
    mocks.emitTo.mockRejectedValueOnce(new Error("event bus unavailable"));

    await expect(
      requestMainAutoEnhance(
        autoEnhanceRequest.sessionId,
        autoEnhanceRequest.mode,
      ),
    ).rejects.toThrow("event bus unavailable");
    expect(mocks.callbacks.has(RESULT_EVENT)).toBe(false);
  });
});

describe("handleMainAutoEnhanceRequest", () => {
  it("acknowledges only after the durable service request completes", async () => {
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);

    await handleMainAutoEnhanceRequest(
      { requestAutoEnhance },
      autoEnhanceRequest,
    );

    expect(requestAutoEnhance).toHaveBeenCalledWith(
      autoEnhanceRequest.sessionId,
      autoEnhanceRequest.mode,
    );
    expect(requestAutoEnhance).toHaveBeenCalledBefore(mocks.emitTo);
    expect(mocks.emitTo).toHaveBeenCalledWith(
      autoEnhanceRequest.sourceLabel,
      RESULT_EVENT,
      {
        requestId: autoEnhanceRequest.requestId,
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
          ...autoEnhanceRequest,
        },
      ),
    ).resolves.toBeUndefined();

    const result = {
      requestId: autoEnhanceRequest.requestId,
      completed: true,
      error: null,
    };
    expect(requestAutoEnhance).toHaveBeenCalledOnce();
    expect(mocks.emitTo).toHaveBeenCalledWith(
      autoEnhanceRequest.sourceLabel,
      RESULT_EVENT,
      result,
    );
    expect(mocks.emit).toHaveBeenCalledWith(RESULT_EVENT, result);
    expect(requestAutoEnhance).toHaveBeenCalledBefore(mocks.emitTo);
    expect(mocks.emitTo).toHaveBeenCalledBefore(mocks.emit);
  });

  it("does not repeat scheduling when both acknowledgement paths fail", async () => {
    mocks.emitTo.mockRejectedValueOnce(new Error("target unavailable"));
    mocks.emit.mockRejectedValueOnce(new Error("broadcast unavailable"));
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);

    await expect(
      handleMainAutoEnhanceRequest({ requestAutoEnhance }, autoEnhanceRequest),
    ).rejects.toThrow("broadcast unavailable");

    expect(requestAutoEnhance).toHaveBeenCalledOnce();
  });

  it.each([
    {
      name: "a main-side error",
      deps: {
        requestAutoEnhance: vi
          .fn()
          .mockRejectedValue(new Error("summary marker write failed")),
      },
      expectedError: "summary marker write failed",
    },
    {
      name: "a missing main enhancer",
      deps: null,
      expectedError: "Main auto-summary service is not ready",
    },
  ])(
    "reports $name to the requesting window",
    async ({ deps, expectedError }) => {
      const request = deps
        ? { ...autoEnhanceRequest, mode: "if_empty" as const }
        : autoEnhanceRequest;
      await handleMainAutoEnhanceRequest(deps, request);

      expect(mocks.emitTo).toHaveBeenCalledWith(
        autoEnhanceRequest.sourceLabel,
        RESULT_EVENT,
        {
          requestId: autoEnhanceRequest.requestId,
          completed: false,
          error: expectedError,
        },
      );
    },
  );
});
