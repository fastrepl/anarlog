import type { TextStreamPart, ToolSet } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { type EarlyValidatorFn, withEarlyValidationRetry } from "./validate";

async function* createMockStream(
  chunks: TextStreamPart<ToolSet>[],
  signal?: AbortSignal,
): AsyncIterable<TextStreamPart<ToolSet>> {
  for (const chunk of chunks) {
    if (signal?.aborted) {
      break;
    }
    yield chunk;
  }
}

async function collectStream<T>(stream: AsyncIterable<T>): Promise<T[]> {
  const results: T[] = [];
  for await (const chunk of stream) {
    results.push(chunk);
  }
  return results;
}

describe("withEarlyValidationRetry", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should pass through stream when validation succeeds on first attempt", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "Hello", id: "1" },
      { type: "text-delta", text: " world", id: "1" },
      { type: "text-delta", text: "!", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = (_text) => ({ valid: true });

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
    expect(executeStream).toHaveBeenCalledTimes(1);
  });

  it("should buffer chunks until minChar threshold is reached", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "H", id: "1" },
      { type: "text-delta", text: "e", id: "1" },
      { type: "text-delta", text: "l", id: "1" },
      { type: "text-delta", text: "l", id: "1" },
      { type: "text-delta", text: "o", id: "1" },
      { type: "text-delta", text: " world", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = vi.fn((_text) => ({
      valid: true as const,
    }));

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
    expect(validator).toHaveBeenCalledWith("Hello");
  });

  it("should retry when validation fails before maxChar", async () => {
    let attemptCount = 0;

    const executeStream = vi.fn((signal: AbortSignal, _context) => {
      attemptCount++;
      const chunks: TextStreamPart<ToolSet>[] =
        attemptCount === 1
          ? [
              { type: "text-delta", text: "Invalid", id: "1" },
              { type: "text-delta", text: " start", id: "1" },
            ]
          : [
              { type: "text-delta", text: "Valid", id: "1" },
              { type: "text-delta", text: " content", id: "1" },
            ];

      return createMockStream(chunks, signal);
    });

    const validator: EarlyValidatorFn = vi.fn((text) => {
      if (text.trim().startsWith("Invalid")) {
        return {
          valid: false as const,
          feedback: "Text must not start with Invalid",
        };
      }
      return { valid: true as const };
    });

    const onRetry = vi.fn();

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
      maxRetries: 2,
      onRetry,
    });

    const results = await collectStream(stream);

    expect(results).toEqual([
      { type: "text-delta", text: "Valid", id: "1" },
      { type: "text-delta", text: " content", id: "1" },
    ]);
    expect(executeStream).toHaveBeenCalledTimes(2);
    expect(onRetry).toHaveBeenCalledWith(1, "Text must not start with Invalid");
  });

  it("should give up and yield output after maxRetries attempts", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "Invalid", id: "1" },
      { type: "text-delta", text: " text", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = vi.fn(() => ({
      valid: false as const,
      feedback: "Always invalid",
    }));

    const onGiveUp = vi.fn();

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
      maxRetries: 2,
      onGiveUp,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
    expect(executeStream).toHaveBeenCalledTimes(2);
    expect(onGiveUp).toHaveBeenCalledTimes(1);
  });

  it("should flush buffer when maxChar threshold is reached", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "Hello ", id: "1" },
      { type: "text-delta", text: "world, ", id: "1" },
      { type: "text-delta", text: "this is a long text ", id: "1" },
      { type: "text-delta", text: "that exceeds maxChar", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = vi.fn(() => ({
      valid: true as const,
    }));

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
  });

  it("should pass previousFeedback to next attempt", async () => {
    let attemptCount = 0;

    const executeStream = vi.fn(
      (
        signal: AbortSignal,
        context: { attempt: number; previousFeedback?: string },
      ) => {
        attemptCount++;
        const chunks: TextStreamPart<ToolSet>[] =
          attemptCount === 1
            ? [
                {
                  type: "text-delta",
                  text: "Wrong answer",
                  id: "1",
                },
              ]
            : [
                {
                  type: "text-delta",
                  text: "Correct answer",
                  id: "1",
                },
              ];

        if (attemptCount === 2) {
          expect(context.previousFeedback).toBe("Must start with Correct");
        }

        return createMockStream(chunks, signal);
      },
    );

    const validator: EarlyValidatorFn = (text) => {
      if (!text.trim().startsWith("Correct")) {
        return { valid: false, feedback: "Must start with Correct" };
      }
      return { valid: true };
    };

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
      maxRetries: 3,
    });

    await collectStream(stream);

    expect(executeStream).toHaveBeenCalledTimes(2);
  });

  it("should handle non-text-delta chunks", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-start", id: "1" },
      { type: "text-delta", text: "Hello world", id: "1" },
      { type: "text-end", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = () => ({ valid: true as const });

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
  });

  it("should trim text when checking minChar threshold", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "   ", id: "1" },
      { type: "text-delta", text: "Hello", id: "1" },
      { type: "text-delta", text: " world", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = vi.fn(() => ({
      valid: true as const,
    }));

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    });

    await collectStream(stream);

    expect(validator).toHaveBeenCalledWith("   Hello");
  });

  it("should give up and yield output when validation fails with maxRetries 1", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "Bad start", id: "1" },
      { type: "text-delta", text: " more text", id: "1" },
      { type: "text-delta", text: " even more", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = () => ({
      valid: false as const,
      feedback: "Bad start",
    });

    const onGiveUp = vi.fn();

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
      maxRetries: 1,
      onGiveUp,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
    expect(onGiveUp).toHaveBeenCalledTimes(1);
  });

  it("should use default options when not provided", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "Hello world", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = () => ({ valid: true as const });

    const stream = withEarlyValidationRetry(executeStream, validator);

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
  });

  it("should validate after stream ends if validation never completed", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "Hi", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = vi.fn(() => ({
      valid: true as const,
    }));

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 10,
      maxChar: 30,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
  });

  it("should handle multiple retry attempts with different feedback", async () => {
    let attemptCount = 0;
    const feedbackHistory: string[] = [];

    const executeStream = vi.fn(
      (
        signal: AbortSignal,
        context: { attempt: number; previousFeedback?: string },
      ) => {
        attemptCount++;
        if (context.previousFeedback) {
          feedbackHistory.push(context.previousFeedback);
        }

        const chunks: TextStreamPart<ToolSet>[] =
          attemptCount === 1
            ? [{ type: "text-delta", text: "First wrong", id: "1" }]
            : attemptCount === 2
              ? [
                  {
                    type: "text-delta",
                    text: "Second wrong",
                    id: "1",
                  },
                ]
              : [
                  {
                    type: "text-delta",
                    text: "Correct answer",
                    id: "1",
                  },
                ];

        return createMockStream(chunks, signal);
      },
    );

    const validator: EarlyValidatorFn = (text) => {
      if (text.includes("First")) {
        return {
          valid: false as const,
          feedback: "First attempt failed",
        };
      }
      if (text.includes("Second")) {
        return {
          valid: false as const,
          feedback: "Second attempt failed",
        };
      }
      return { valid: true as const };
    };

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
      maxRetries: 3,
    });

    const results = await collectStream(stream);

    expect(results).toEqual([
      { type: "text-delta", text: "Correct answer", id: "1" },
    ]);
    expect(feedbackHistory).toEqual([
      "First attempt failed",
      "Second attempt failed",
    ]);
    expect(executeStream).toHaveBeenCalledTimes(3);
  });

  it("should handle validation that passes after accumulating exactly minChar", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-delta", text: "12345", id: "1" },
      { type: "text-delta", text: "67890", id: "1" },
    ];

    const executeStream = vi.fn((signal: AbortSignal) =>
      createMockStream(chunks, signal),
    );

    const validator: EarlyValidatorFn = vi.fn(() => ({
      valid: true as const,
    }));

    const stream = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    });

    const results = await collectStream(stream);

    expect(results).toEqual(chunks);
    expect(validator).toHaveBeenCalledTimes(1);
    expect(validator).toHaveBeenCalledWith("12345");
  });

  it("emits buffered chunks immediately after validation succeeds", async () => {
    const chunks: TextStreamPart<ToolSet>[] = [
      { type: "text-start", id: "1" },
      { type: "text-delta", text: "Hello", id: "1" },
      { type: "text-delta", text: " world", id: "1" },
      { type: "text-end", id: "1" },
    ];

    async function* executeStream(signal: AbortSignal) {
      for (const chunk of chunks) {
        if (signal.aborted) {
          return;
        }
        yield chunk;
      }
    }

    const validator: EarlyValidatorFn = () => ({ valid: true });

    const iterator = withEarlyValidationRetry(executeStream, validator, {
      minChar: 5,
      maxChar: 30,
    })[Symbol.asyncIterator]();

    const first = await iterator.next();
    expect(first).toEqual({
      done: false,
      value: { type: "text-start", id: "1" },
    });

    const second = await iterator.next();
    expect(second).toEqual({
      done: false,
      value: { type: "text-delta", text: "Hello", id: "1" },
    });

    const third = await iterator.next();
    expect(third).toEqual({
      done: false,
      value: { type: "text-delta", text: " world", id: "1" },
    });

    const fourth = await iterator.next();
    expect(fourth).toEqual({
      done: false,
      value: { type: "text-end", id: "1" },
    });

    const completion = await iterator.next();
    expect(completion).toEqual({ done: true, value: undefined });
  });
});
