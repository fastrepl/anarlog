import { APICallError } from "ai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TASK_CONFIGS } from "./task-configs";
import {
  createTasksSlice,
  extractUnderlyingError,
  getTaskStreamStartTimeoutMs,
  isRetryableAIError,
  MAX_AI_TASK_STREAM_CHARACTERS,
  MAX_RETAINED_AI_TASKS,
  TASK_STREAM_IDLE_TIMEOUT_MS,
  TASK_STREAM_LOCAL_START_TIMEOUT_MS,
  TASK_STREAM_START_TIMEOUT_MS,
} from "./tasks";

const mocks = vi.hoisted(() => ({
  getStoredSettingValues: vi.fn(async () => ({
    values: {},
    hasValues: new Set(),
  })),
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: mocks.getStoredSettingValues,
}));

const originalEnhanceConfig = { ...TASK_CONFIGS.enhance };

beforeEach(() => {
  mocks.getStoredSettingValues.mockReset();
  mocks.getStoredSettingValues.mockResolvedValue({
    values: {},
    hasValues: new Set(),
  });
});

afterEach(() => {
  vi.useRealTimers();
  Object.assign(TASK_CONFIGS.enhance, originalEnhanceConfig);
});

describe("createTasksSlice", () => {
  it("hydrates a remote task snapshot without an abort controller", () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    const taskId = "summary-1-enhance" as const;
    state.syncRemoteTask(taskId, {
      taskType: "enhance",
      status: "generating",
      streamedText: "Generated",
      currentStep: { type: "generating" },
    });

    expect(state.tasks[taskId]).toMatchObject({
      taskType: "enhance",
      status: "generating",
      streamedText: "Generated",
      currentStep: { type: "generating" },
      abortController: null,
    });
  });

  it("retains only the newest bounded remote task snapshots", () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    for (let index = 0; index <= MAX_RETAINED_AI_TASKS; index += 1) {
      state.syncRemoteTask(`summary-${index}-enhance`, {
        taskType: "enhance",
        status: "success",
        streamedText: `summary-${index}`,
      });
    }

    expect(Object.keys(state.tasks)).toHaveLength(MAX_RETAINED_AI_TASKS);
    expect(state.tasks["summary-0-enhance"]).toBeUndefined();
    expect(
      state.tasks[`summary-${MAX_RETAINED_AI_TASKS}-enhance`]?.streamedText,
    ).toBe(`summary-${MAX_RETAINED_AI_TASKS}`);
  });

  it("caps synchronized task text", () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    state.syncRemoteTask("summary-1-enhance", {
      taskType: "enhance",
      status: "success",
      streamedText: "x".repeat(MAX_AI_TASK_STREAM_CHARACTERS + 1),
    });

    expect(state.tasks["summary-1-enhance"]?.streamedText).toHaveLength(
      MAX_AI_TASK_STREAM_CHARACTERS,
    );
  });

  it("keeps a task generating until onSuccess finishes", async () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

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
    state = createTasksSlice(set, get);

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

  it("marks a task failed when durable post-processing fails", async () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
    });
    TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {
      throw new Error("database write failed");
    });

    const taskId = "session-1-enhance" as const;
    await state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    expect(state.tasks[taskId]).toMatchObject({
      status: "error",
      streamedText: "",
      error: expect.objectContaining({ message: "database write failed" }),
    });
  });

  it("retries database contention before starting generation", async () => {
    vi.useFakeTimers();
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi
      .fn()
      .mockRejectedValueOnce(
        new Error("error returned from database: (code: 5) database is locked"),
      )
      .mockResolvedValue({} as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
    });
    TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {});

    const taskId = "session-locked-read-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    await vi.runAllTimersAsync();
    await promise;

    expect(TASK_CONFIGS.enhance.transformArgs).toHaveBeenCalledTimes(2);
    expect(TASK_CONFIGS.enhance.executeWorkflow).toHaveBeenCalledOnce();
    expect(state.tasks[taskId]).toMatchObject({
      status: "success",
      streamedText: "Generated summary",
    });
  });

  it("retries summary persistence without generating it twice", async () => {
    vi.useFakeTimers();
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
    });
    TASK_CONFIGS.enhance.onSuccess = vi
      .fn()
      .mockRejectedValueOnce(new Error("database table is locked"))
      .mockResolvedValue(undefined);

    const taskId = "session-locked-write-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    await vi.runAllTimersAsync();
    await promise;

    expect(TASK_CONFIGS.enhance.executeWorkflow).toHaveBeenCalledOnce();
    expect(TASK_CONFIGS.enhance.onSuccess).toHaveBeenCalledTimes(2);
    expect(state.tasks[taskId]).toMatchObject({
      status: "success",
      streamedText: "Generated summary",
    });
  });

  it("persists streamed text when the provider never closes the stream", async () => {
    vi.useFakeTimers();
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
      await new Promise(() => {});
    });
    TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {});

    const taskId = "session-idle-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    await vi.waitFor(() => {
      expect(state.tasks[taskId]?.streamedText).toBe("Generated summary");
    });
    await vi.advanceTimersByTimeAsync(TASK_STREAM_IDLE_TIMEOUT_MS);
    await promise;

    expect(TASK_CONFIGS.enhance.onSuccess).toHaveBeenCalledWith(
      expect.objectContaining({ text: "Generated summary" }),
    );
    expect(state.tasks[taskId]).toMatchObject({
      status: "success",
      streamedText: "Generated summary",
    });
  });

  it("does not persist streamed text after cancellation during the idle timeout", async () => {
    vi.useFakeTimers();
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield { type: "text-delta", text: "Generated summary" } as any;
      await new Promise(() => {});
    });
    TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {});

    const taskId = "session-cancelled-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    await vi.waitFor(() => {
      expect(state.tasks[taskId]?.streamedText).toBe("Generated summary");
    });
    state.cancel(taskId);
    await vi.advanceTimersByTimeAsync(TASK_STREAM_IDLE_TIMEOUT_MS);
    await promise;

    expect(TASK_CONFIGS.enhance.onSuccess).not.toHaveBeenCalled();
    expect(state.tasks[taskId]).toMatchObject({ status: "idle" });
  });

  it("fails a task when the provider never returns text", async () => {
    vi.useFakeTimers();
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      await new Promise(() => {});
    });
    TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {});

    const taskId = "session-start-timeout-enhance" as const;
    const promise = state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(TASK_STREAM_START_TIMEOUT_MS);
    await promise;

    expect(TASK_CONFIGS.enhance.onSuccess).not.toHaveBeenCalled();
    expect(state.tasks[taskId]).toMatchObject({
      status: "error",
      error: expect.objectContaining({
        message: "AI generation did not return any text.",
      }),
    });
  });

  it("stops generation before retaining an oversized output", async () => {
    let state: ReturnType<typeof createTasksSlice>;
    const set = (updater: any) => {
      state =
        typeof updater === "function"
          ? updater(state)
          : { ...state, ...updater };
    };
    const get = () => state;
    state = createTasksSlice(set, get);

    TASK_CONFIGS.enhance.transformArgs = vi.fn(async () => ({}) as any);
    TASK_CONFIGS.enhance.transforms = [];
    TASK_CONFIGS.enhance.executeWorkflow = vi.fn(async function* () {
      yield {
        type: "text-delta",
        text: "x".repeat(MAX_AI_TASK_STREAM_CHARACTERS + 1),
      } as any;
    });
    TASK_CONFIGS.enhance.onSuccess = vi.fn(async () => {});

    const taskId = "oversized-enhance" as const;
    await state.generate(taskId, {
      model: {} as any,
      taskType: "enhance",
      args: { sessionId: "session-1", enhancedNoteId: "note-1" },
    });

    expect(TASK_CONFIGS.enhance.onSuccess).not.toHaveBeenCalled();
    expect(state.tasks[taskId]).toMatchObject({
      status: "error",
      streamedText: "",
      error: expect.objectContaining({
        message: "AI generation exceeded the safe output limit.",
      }),
    });
  });
});

describe("getTaskStreamStartTimeoutMs", () => {
  it.each(["ollama.chat", "lmstudio.chat", "unsloth.chat", "apple_foundation"])(
    "allows local provider %s more time to start",
    (provider) => {
      expect(getTaskStreamStartTimeoutMs({ provider } as any)).toBe(
        TASK_STREAM_LOCAL_START_TIMEOUT_MS,
      );
    },
  );

  it("keeps the standard start timeout for remote providers", () => {
    expect(
      getTaskStreamStartTimeoutMs({ provider: "openai.chat" } as any),
    ).toBe(TASK_STREAM_START_TIMEOUT_MS);
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

describe("isRetryableAIError", () => {
  it("classifies API call errors that lost their prototype by shape", () => {
    const error = new Error(
      `Bad Request ${"transcript mentioning a timeout ".repeat(100)}`,
    );
    error.name = "AI_APICallError";
    (error as any).isRetryable = false;
    (error as any).statusCode = 400;

    expect(isRetryableAIError(error)).toBe(false);
  });

  it("keeps retryable shape-matched API call errors retryable", () => {
    const error = new Error("Overloaded");
    error.name = "AI_APICallError";
    (error as any).isRetryable = false;
    (error as any).statusCode = 529;

    expect(isRetryableAIError(error)).toBe(true);
  });

  it("does not pattern-match transient words inside long messages", () => {
    const error = new Error(
      `Invalid prompt: ${"the deploy timed out around 429 requests ".repeat(50)}`,
    );

    expect(isRetryableAIError(error)).toBe(false);
  });

  it("pattern-matches transient words in concise messages", () => {
    expect(isRetryableAIError(new Error("Request timed out"))).toBe(true);
  });
});
