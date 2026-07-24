import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS,
  CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS,
  CHAT_CLOUDSYNC_END_RETRY_INTERVAL_MS,
  CHAT_CLOUDSYNC_RELEASE_DELAY_MS,
  createChatCloudsyncActivityController,
  guardChatTransport,
} from "./cloudsync-activity";

function sequentialAttemptKeys() {
  let attempt = 0;
  return (logicalKey: string) => `${logicalKey}:attempt-${++attempt}`;
}

describe("chat CloudSync activity", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("keeps a turn paused through the cancellable trailing delay", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await expect(controller.start("turn-1")).resolves.toMatchObject({
      key: "turn-1:attempt-1",
    });
    controller.finish("turn-1");

    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS - 1);
    expect(end).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(begin).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("debounces rapid turns without allowing a sync gap", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.start("turn-1");
    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(500);

    await controller.start("turn-2");
    await vi.advanceTimersByTimeAsync(500);
    expect(end).not.toHaveBeenCalled();

    controller.finish("turn-2");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(begin.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-2:attempt-2"],
    ]);
    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-2:attempt-2"],
    ]);
  });

  it("keeps overlapping attempts for the same user turn independent", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.start("turn-1");
    await controller.start("turn-1");
    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS * 2);
    expect(end).not.toHaveBeenCalled();

    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-1:attempt-2"],
    ]);
  });

  it("does not let an old delayed end release a new same-key attempt", async () => {
    let finishOldEnd: (() => void) | undefined;
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi
      .fn()
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          finishOldEnd = resolve;
        }),
      )
      .mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.start("turn-1");
    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");

    await controller.start("turn-1");
    expect(begin).toHaveBeenLastCalledWith("chat", "turn-1:attempt-2");
    finishOldEnd?.();
    await Promise.resolve();
    expect(end).not.toHaveBeenCalledWith("chat", "turn-1:attempt-2");

    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-2");
  });

  it("cleans up a native lease when acquisition fails", async () => {
    const beginError = new Error("native bridge unavailable");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(beginError)
      .mockResolvedValueOnce(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await expect(controller.start("failed-turn")).rejects.toBe(beginError);
    expect(end).toHaveBeenCalledWith("chat", "failed-turn:attempt-1");

    await expect(controller.start("next-turn")).resolves.toMatchObject({
      key: "next-turn:attempt-2",
    });
    controller.finish("next-turn");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "next-turn:attempt-2");
  });

  it("does not start transport when lease acquisition fails", async () => {
    const beginError = new Error("native bridge unavailable");
    const begin = vi.fn().mockRejectedValue(beginError);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const sendMessages = vi.fn();
    const transport = guardChatTransport(
      {
        sendMessages,
        reconnectToStream: vi.fn().mockResolvedValue(null),
      },
      activity,
    );

    await expect(
      transport.sendMessages({
        trigger: "submit-message",
        chatId: "chat-1",
        messageId: undefined,
        messages: [{ id: "turn-1", role: "user", parts: [] }],
        abortSignal: new AbortController().signal,
      }),
    ).rejects.toBe(beginError);

    expect(sendMessages).not.toHaveBeenCalled();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("runs guarded preflight only after lease acquisition succeeds", async () => {
    let acquireLease: (() => void) | undefined;
    const begin = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          acquireLease = resolve;
        }),
    );
    const activity = createChatCloudsyncActivityController({
      begin,
      end: vi.fn().mockResolvedValue(undefined),
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi.fn().mockResolvedValue(undefined);
    const sendMessages = vi.fn().mockResolvedValue(
      new ReadableStream({
        start: (controller) => controller.close(),
      }),
    );
    const transport = guardChatTransport(
      {
        sendMessages,
        reconnectToStream: vi.fn().mockResolvedValue(null),
      },
      activity,
      { beforeSend: () => preflight },
    );

    const sent = transport.sendMessages({
      trigger: "submit-message",
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user", parts: [] }],
      abortSignal: new AbortController().signal,
    });
    await Promise.resolve();
    expect(preflight).not.toHaveBeenCalled();
    expect(sendMessages).not.toHaveBeenCalled();

    acquireLease?.();
    await sent;
    expect(preflight).toHaveBeenCalledOnce();
    expect(sendMessages).toHaveBeenCalledOnce();
  });

  it("keeps guard failures bound to their own completion callback", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const transportError = new Error("preflight failed");
    const firstPreflight = vi.fn().mockRejectedValue(transportError);
    const secondPreflight = vi.fn().mockResolvedValue(undefined);
    const preflights = [firstPreflight, secondPreflight];
    const transport = guardChatTransport(
      {
        sendMessages: vi.fn().mockResolvedValue(
          new ReadableStream({
            start: (controller) => controller.close(),
          }),
        ),
        reconnectToStream: vi.fn().mockResolvedValue(null),
      },
      activity,
      { beforeSend: () => preflights.shift() },
    );
    const options = {
      trigger: "submit-message" as const,
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user" as const, parts: [] }],
      abortSignal: new AbortController().signal,
    };

    await expect(transport.sendMessages(options)).rejects.toBe(transportError);
    await expect(transport.sendMessages(options)).resolves.toBeInstanceOf(
      ReadableStream,
    );

    activity.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).not.toHaveBeenCalled();

    activity.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-1:attempt-2"],
    ]);
  });

  it("waits for active work before disposing", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.start("turn-1");
    const disposed = controller.dispose();
    await vi.advanceTimersByTimeAsync(
      CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS - 1,
    );
    expect(end).not.toHaveBeenCalled();

    controller.finish("turn-1");
    await disposed;
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("bounds disposal when completion callbacks never arrive", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.start("turn-1");
    const disposed = controller.dispose();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS);
    await disposed;

    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("can resume after a StrictMode-style empty cleanup", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.dispose();
    controller.resume();

    await expect(controller.start("turn-1")).resolves.toMatchObject({
      key: "turn-1:attempt-1",
    });
  });

  it("can resume while a previous mount's lease is still draining", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.start("turn-1");
    const disposed = controller.dispose();
    controller.resume();

    await expect(controller.start("turn-2")).resolves.toMatchObject({
      key: "turn-2:attempt-2",
    });
    await expect(disposed).resolves.toBeUndefined();

    controller.finish("turn-1");
    controller.finish("turn-2");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-2:attempt-2"],
    ]);
  });

  it("keeps dispose idempotent after cleanup has settled", async () => {
    const controller = createChatCloudsyncActivityController({
      begin: vi.fn().mockResolvedValue(undefined),
      end: vi.fn().mockResolvedValue(undefined),
      createAttemptKey: sequentialAttemptKeys(),
    });

    const firstDispose = controller.dispose();
    await firstDispose;
    const secondDispose = controller.dispose();

    expect(secondDispose).toBe(firstDispose);
    await expect(secondDispose).resolves.toBeUndefined();
  });

  it("retries transient native release failures", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi
      .fn()
      .mockRejectedValueOnce(new Error("bridge busy"))
      .mockResolvedValue(undefined);
    const onReleaseError = vi.fn();
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
      onReleaseError,
    });

    await controller.start("turn-1");
    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(
      CHAT_CLOUDSYNC_RELEASE_DELAY_MS + CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS[0],
    );

    expect(end).toHaveBeenCalledTimes(2);
    expect(onReleaseError).not.toHaveBeenCalled();
  });

  it("keeps retrying persistent release failures without another chat", async () => {
    const persistentError = new Error("bridge unavailable");
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockRejectedValue(persistentError);
    const onReleaseError = vi.fn();
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
      onReleaseError,
    });

    await controller.start("turn-1");
    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(
      CHAT_CLOUDSYNC_RELEASE_DELAY_MS +
        CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS.reduce(
          (total, delay) => total + delay,
          0,
        ),
    );

    expect(end).toHaveBeenCalledTimes(
      CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS.length + 1,
    );
    expect(onReleaseError).toHaveBeenCalledWith(persistentError);

    end.mockResolvedValue(undefined);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_END_RETRY_INTERVAL_MS);

    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
    expect(end).toHaveBeenCalledTimes(
      CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS.length + 2,
    );
  });
});
