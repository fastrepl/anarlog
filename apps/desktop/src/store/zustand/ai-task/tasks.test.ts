import { APICallError } from "ai";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TASK_CONFIGS } from "./task-configs";
import { createTasksSlice, extractUnderlyingError } from "./tasks";

const originalEnhanceConfig = { ...TASK_CONFIGS.enhance };

afterEach(() => {
  Object.assign(TASK_CONFIGS.enhance, originalEnhanceConfig);
});

describe("createTasksSlice", () => {
  it("keeps a task generating until onSuccess finishes", async () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get, {
      persistedStore: {} as any,
      settingsStore: {} as any,
    });

    let resolveOnSuccess: () => void;
    const onSuccessStarted = new Promise<void>((resolve) => {
      TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {
        resolve();
        await new Promise<void>((innerResolve) => {
          resolveOnSuccess = innerResolve;
        });
      });
    });

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
    });

    const taskId = "session-1-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
    });

    await onSuccessStarted;

    expect(state.tasks[taskId]).toMatchObject({
      status: "generating",
      streamedText: "Generated summary",
    });

    resolveOnSuccess!();
    await promise;

    expect(state.tasks[taskId]).toMatchObject({
      status: "success",
      streamedText: "Generated summary",
    });
  });

  it("does not mark a task successful when it is cancelled during onSuccess", async () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get, {
      persistedStore: {} as any,
      settingsStore: {} as any,
    });

    let resolveOnSuccess: () => void;
    const onSuccessStarted = new Promise<void>((resolve) => {
      TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {
        resolve();
        await new Promise<void>((innerResolve) => {
          resolveOnSuccess = innerResolve;
        });
      });
    });

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
    });

    const taskId = "session-1-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
    });

    await onSuccessStarted;
    state.cancel(taskId);
    resolveOnSuccess!();
    await promise;

    expect(state.tasks[taskId]).toMatchObject({
      status: "idle",
    });
  });
});

describe("extractUnderlyingError", () => {
  it("normalizes exhausted provider overload retries", () => {
    const retryError = new Error(
      "Failed after 3 attempts. Last error: Overloaded",
    );
    retryError.name = "AI_RetryError";
    (retryError as any).lastError = new Error("Overloaded");

    expect(extractUnderlyingError(retryError).message).toBe(
      "The AI model is overloaded right now. Wait a moment, then retry.",
    );
  });

  it("normalizes retryable API call failures", () => {
    const error = new APICallError({
      message: "Service unavailable",
      url: "https://example.com",
      requestBodyValues: {},
      statusCode: 503,
    });

    expect(extractUnderlyingError(error).message).toBe(
      "The AI model is overloaded right now. Wait a moment, then retry.",
    );
  });

  it("preserves API conflict errors", () => {
    const error = new APICallError({
      message: "Conflict",
      url: "https://example.com",
      requestBodyValues: {},
      statusCode: 409,
    });

    expect(extractUnderlyingError(error)).toBe(error);
  });

  it("preserves non-transient errors", () => {
    const error = new Error("Invalid API key");

    expect(extractUnderlyingError(error)).toBe(error);
  });
});
