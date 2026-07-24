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

  it("keeps the lease until tracked side writes and the trailing delay settle", async () => {
    let finishSideWrite: (() => void) | undefined;
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const attempt = await controller.start("turn-1");
    attempt?.trackCompletion(
      new Promise<void>((resolve) => {
        finishSideWrite = resolve;
      }),
    );

    attempt?.finish();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS * 2);
    expect(end).not.toHaveBeenCalled();

    finishSideWrite?.();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS - 1);
    expect(end).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
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

    await expect(controller.start("failed-turn")).resolves.toMatchObject({
      key: "failed-turn:attempt-2",
    });
    controller.finish("failed-turn");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "failed-turn:attempt-2");
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

  it("releases the exact attempt when transport rejects before streaming", async () => {
    const transportError = new Error("transport unavailable");
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin: vi.fn().mockResolvedValue(undefined),
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const transport = guardChatTransport(
      {
        sendMessages: vi.fn().mockRejectedValue(transportError),
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
    ).rejects.toBe(transportError);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("releases the exact attempt when aborted after acquisition", async () => {
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin: vi.fn().mockResolvedValue(undefined),
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
    const abortController = new AbortController();
    abortController.abort();

    await expect(
      transport.sendMessages({
        trigger: "submit-message",
        chatId: "chat-1",
        messageId: undefined,
        messages: [{ id: "turn-1", role: "user", parts: [] }],
        abortSignal: abortController.signal,
      }),
    ).rejects.toMatchObject({ name: "AbortError" });
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

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
      {
        beforeSend: () => ({
          run: preflight,
          persistOnCancel: true,
        }),
      },
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
      {
        beforeSend: () => {
          const preflight = preflights.shift();
          return preflight
            ? { run: preflight, persistOnCancel: true }
            : undefined;
        },
      },
    );
    const options = {
      trigger: "submit-message" as const,
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user" as const, parts: [] }],
      abortSignal: new AbortController().signal,
    };

    await expect(transport.sendMessages(options)).rejects.toBe(transportError);
    activity.finish("turn-1");
    await expect(transport.sendMessages(options)).resolves.toBeInstanceOf(
      ReadableStream,
    );

    activity.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-1:attempt-2"],
    ]);
  });

  it("reruns a failed preflight on its lease before finished persistence", async () => {
    let finishRepair: (() => void) | undefined;
    let finishTrackedWrite: (() => void) | undefined;
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflightError = new Error("preflight failed");
    const preflight = vi
      .fn()
      .mockRejectedValueOnce(preflightError)
      .mockImplementationOnce(
        (trackCompletion: (completion: Promise<unknown>) => void) => {
          trackCompletion(
            new Promise<void>((resolve) => {
              finishTrackedWrite = resolve;
            }),
          );
          return Promise.resolve();
        },
      );

    await expect(
      activity.start("turn-1", {
        preflight: {
          run: preflight,
          persistOnCancel: true,
        },
      }),
    ).rejects.toBe(preflightError);

    const repair = activity.runWithLease(
      "turn-1",
      () =>
        new Promise<void>((resolve) => {
          finishRepair = resolve;
        }),
    );
    await Promise.resolve();
    expect(preflight).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS * 2);
    expect(end).not.toHaveBeenCalled();

    finishRepair?.();
    await repair;
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).not.toHaveBeenCalled();

    finishTrackedWrite?.();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(begin).toHaveBeenCalledOnce();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("replays an unstarted preflight only after durable reacquisition", async () => {
    let acquireRepair: (() => void) | undefined;
    let finishTrackedWrite: (() => void) | undefined;
    const beginError = new Error("activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(beginError)
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          acquireRepair = resolve;
        }),
      );
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const events: string[] = [];
    const preflight = vi.fn(
      (trackCompletion: (completion: Promise<unknown>) => void) => {
        events.push("preflight");
        trackCompletion(
          new Promise<void>((resolve) => {
            finishTrackedWrite = resolve;
          }),
        );
      },
    );
    const persistFinished = vi.fn(() => {
      events.push("finished");
    });

    await expect(
      activity.start("turn-1", {
        preflight: { run: preflight, persistOnCancel: true },
      }),
    ).rejects.toBe(beginError);

    const repair = activity.runWithLease("turn-1", persistFinished);
    await Promise.resolve();
    expect(preflight).not.toHaveBeenCalled();
    expect(persistFinished).not.toHaveBeenCalled();

    acquireRepair?.();
    await repair;
    expect(events).toEqual(["preflight", "finished"]);
    expect(end).not.toHaveBeenCalledWith("chat", "turn-1:attempt-2");

    finishTrackedWrite?.();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-2");
  });

  it("does not let a failed repair ticket steal the next same-key retry", async () => {
    const beginError = new Error("activity drain timed out");
    const preflightError = new Error("preflight failed");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(beginError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const failedPreflight = vi.fn().mockRejectedValue(preflightError);
    const failedFinishedPersist = vi.fn();

    await expect(
      activity.start("turn-1", {
        preflight: {
          run: failedPreflight,
          persistOnCancel: true,
        },
      }),
    ).rejects.toBe(beginError);
    await expect(
      activity.runWithLease("turn-1", failedFinishedPersist),
    ).rejects.toBe(preflightError);

    const retryPreflight = vi.fn().mockResolvedValue(undefined);
    await activity.start("turn-1", {
      preflight: {
        run: retryPreflight,
        persistOnCancel: true,
      },
    });
    const retryFinishedPersist = vi.fn().mockResolvedValue(undefined);
    await activity.runWithLease("turn-1", retryFinishedPersist);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(failedPreflight).toHaveBeenCalledOnce();
    expect(failedFinishedPersist).not.toHaveBeenCalled();
    expect(retryPreflight).toHaveBeenCalledOnce();
    expect(retryFinishedPersist).toHaveBeenCalledOnce();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-3");
  });

  it("repairs a guarded preflight before a same-key retry can proceed", async () => {
    const beginError = new Error("activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(beginError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const firstPreflight = vi.fn().mockResolvedValue(undefined);
    const retryPreflight = vi.fn().mockResolvedValue(undefined);
    const preflights = [firstPreflight, retryPreflight];
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
      {
        beforeSend: () => {
          const preflight = preflights.shift();
          return preflight
            ? { run: preflight, persistOnCancel: true }
            : undefined;
        },
      },
    );
    const options = {
      trigger: "submit-message" as const,
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user" as const, parts: [] }],
      abortSignal: new AbortController().signal,
    };

    await expect(transport.sendMessages(options)).rejects.toBe(beginError);
    expect(firstPreflight).toHaveBeenCalledOnce();
    expect(sendMessages).not.toHaveBeenCalled();

    await expect(transport.sendMessages(options)).resolves.toBeInstanceOf(
      ReadableStream,
    );
    expect(retryPreflight).toHaveBeenCalledOnce();
    expect(sendMessages).toHaveBeenCalledOnce();

    activity.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-3");
  });

  it("restores the preflight ticket when guarded reacquisition fails", async () => {
    const initialError = new Error("initial activity drain timed out");
    const repairError = new Error("repair activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(initialError)
      .mockRejectedValueOnce(repairError)
      .mockResolvedValueOnce(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end: vi.fn().mockResolvedValue(undefined),
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi.fn().mockResolvedValue(undefined);
    const transport = guardChatTransport(
      {
        sendMessages: vi.fn(),
        reconnectToStream: vi.fn().mockResolvedValue(null),
      },
      activity,
      {
        beforeSend: () => ({
          run: preflight,
          persistOnCancel: true,
        }),
      },
    );
    const options = {
      trigger: "submit-message" as const,
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user" as const, parts: [] }],
      abortSignal: new AbortController().signal,
    };

    await expect(transport.sendMessages(options)).rejects.toBe(initialError);
    expect(preflight).not.toHaveBeenCalled();

    const persistFinished = vi.fn().mockResolvedValue(undefined);
    await activity.runWithLease("turn-1", persistFinished);

    expect(preflight).toHaveBeenCalledOnce();
    expect(persistFinished).toHaveBeenCalledOnce();
    expect(preflight.mock.invocationCallOrder[0]).toBeLessThan(
      persistFinished.mock.invocationCallOrder[0] ?? 0,
    );
  });

  it("binds a retained preflight ticket to the active same-key retry", async () => {
    const initialError = new Error("initial activity drain timed out");
    const repairError = new Error("repair activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(initialError)
      .mockRejectedValueOnce(repairError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi.fn().mockResolvedValue(undefined);

    await expect(
      activity.start("turn-1", {
        preflight: { run: preflight, persistOnCancel: true },
      }),
    ).rejects.toBe(initialError);
    await expect(activity.runWithLease("turn-1", () => undefined)).rejects.toBe(
      repairError,
    );

    await activity.start("turn-1");
    const persistFinished = vi.fn().mockResolvedValue(undefined);
    await activity.runWithLease("turn-1", persistFinished);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(begin).toHaveBeenCalledTimes(3);
    expect(preflight).toHaveBeenCalledOnce();
    expect(persistFinished).toHaveBeenCalledOnce();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-3");
  });

  it("keeps a transferred preflight recoverable when its replay fails", async () => {
    const initialError = new Error("initial activity drain timed out");
    const replayError = new Error("preflight replay failed");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(initialError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi
      .fn()
      .mockRejectedValueOnce(replayError)
      .mockResolvedValueOnce(undefined);

    await expect(
      activity.start("turn-1", {
        preflight: { run: preflight, persistOnCancel: true },
      }),
    ).rejects.toBe(initialError);
    await activity.start("turn-1");
    await expect(activity.runWithLease("turn-1", () => undefined)).rejects.toBe(
      replayError,
    );
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    const persistFinished = vi.fn().mockResolvedValue(undefined);
    await activity.runWithLease("turn-1", persistFinished);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(begin).toHaveBeenCalledTimes(3);
    expect(preflight).toHaveBeenCalledTimes(2);
    expect(persistFinished).toHaveBeenCalledOnce();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-3");
  });

  it("finishes the active same-key retry ahead of a retained ticket", async () => {
    const initialError = new Error("initial activity drain timed out");
    const repairError = new Error("repair activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(initialError)
      .mockRejectedValueOnce(repairError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await expect(
      activity.start("turn-1", {
        preflight: {
          run: vi.fn().mockResolvedValue(undefined),
          persistOnCancel: true,
        },
      }),
    ).rejects.toBe(initialError);
    await expect(activity.runWithLease("turn-1", () => undefined)).rejects.toBe(
      repairError,
    );

    await activity.start("turn-1");
    activity.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(begin).toHaveBeenCalledTimes(3);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-3");
  });

  it("keeps a retained preflight after an early finish without an active request", async () => {
    const beginError = new Error("activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(beginError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi.fn().mockResolvedValue(undefined);

    await expect(
      activity.start("turn-1", {
        preflight: { run: preflight, persistOnCancel: true },
      }),
    ).rejects.toBe(beginError);

    activity.finish("turn-1");
    const persistFinished = vi.fn().mockResolvedValue(undefined);
    await activity.runWithLease("turn-1", persistFinished);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(preflight).toHaveBeenCalledOnce();
    expect(persistFinished).toHaveBeenCalledOnce();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-2");
  });

  it("does not let a consumed same-key lease own a newer callback", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    const failedAttempt = await activity.start("turn-1");
    failedAttempt?.finish();
    await activity.start("turn-1");

    const persistFinished = vi.fn().mockResolvedValue(undefined);
    await activity.runWithLease("turn-1", persistFinished);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(begin).toHaveBeenCalledTimes(2);
    expect(persistFinished).toHaveBeenCalledOnce();
    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-1:attempt-2"],
    ]);
  });

  it("discards a settled failed preflight after a newer one succeeds", async () => {
    const begin = vi.fn().mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const events: string[] = [];
    const failedPreflight = vi.fn(() => {
      events.push("failed");
      return Promise.reject(new Error("preflight failed"));
    });

    await expect(
      activity.start("turn-1", {
        preflight: { run: failedPreflight, persistOnCancel: true },
      }),
    ).rejects.toThrow("preflight failed");
    await activity.start("turn-1", {
      preflight: {
        persistOnCancel: true,
        run: () => {
          events.push("new");
        },
      },
    });
    await activity.runWithLease("turn-1", () => {
      events.push("finished");
    });
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(events).toEqual(["failed", "new", "finished"]);
    expect(failedPreflight).toHaveBeenCalledOnce();
    expect(begin).toHaveBeenCalledTimes(2);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-2");
  });

  it("lets a successful same-key preflight supersede an inactive failed ticket", async () => {
    const initialError = new Error("initial activity drain timed out");
    const repairError = new Error("repair activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(initialError)
      .mockRejectedValueOnce(repairError)
      .mockResolvedValue(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const failedPreflight = vi.fn();
    const retryPreflight = vi.fn().mockResolvedValue(undefined);
    const preflights = [failedPreflight, retryPreflight];
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
      {
        beforeSend: () => {
          const preflight = preflights.shift();
          return preflight
            ? { run: preflight, persistOnCancel: true }
            : undefined;
        },
      },
    );
    const options = {
      trigger: "submit-message" as const,
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user" as const, parts: [] }],
      abortSignal: new AbortController().signal,
    };

    await expect(transport.sendMessages(options)).rejects.toBe(initialError);
    expect(failedPreflight).not.toHaveBeenCalled();

    await expect(transport.sendMessages(options)).resolves.toBeInstanceOf(
      ReadableStream,
    );
    expect(retryPreflight).toHaveBeenCalledOnce();

    activity.finish("turn-1");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-3");
  });

  it("keeps fresh durable repair acquisition alive through disposal", async () => {
    let acquireRepair: (() => void) | undefined;
    const beginError = new Error("activity drain timed out");
    const begin = vi
      .fn()
      .mockRejectedValueOnce(beginError)
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          acquireRepair = resolve;
        }),
      );
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const repair = vi.fn().mockResolvedValue(undefined);

    await expect(activity.start("turn-1")).rejects.toBe(beginError);
    const repairing = activity.runWithLease("turn-1", repair);
    await Promise.resolve();
    const disposed = activity.dispose(Promise.resolve());

    acquireRepair?.();
    await repairing;
    await vi.advanceTimersByTimeAsync(0);
    await disposed;

    expect(repair).toHaveBeenCalledOnce();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-2");
  });

  it("finishes durable preflight work after disposal wins lease acquisition", async () => {
    let acquireLease: (() => void) | undefined;
    let finishPersist: (() => void) | undefined;
    let finishTrackedWrite: (() => void) | undefined;
    const begin = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          acquireLease = resolve;
        }),
    );
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi.fn(
      (trackCompletion: (completion: Promise<unknown>) => void) => {
        trackCompletion(
          new Promise<void>((resolve) => {
            finishTrackedWrite = resolve;
          }),
        );
        return new Promise<void>((resolve) => {
          finishPersist = resolve;
        });
      },
    );
    const sendMessages = vi.fn();
    const transport = guardChatTransport(
      {
        sendMessages,
        reconnectToStream: vi.fn().mockResolvedValue(null),
      },
      activity,
      {
        beforeSend: () => ({
          run: preflight,
          persistOnCancel: true,
        }),
      },
    );

    const sending = transport.sendMessages({
      trigger: "submit-message",
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user", parts: [] }],
      abortSignal: new AbortController().signal,
    });
    await Promise.resolve();
    const disposed = activity.dispose();

    acquireLease?.();
    await vi.waitFor(() => expect(preflight).toHaveBeenCalledOnce());
    expect(sendMessages).not.toHaveBeenCalled();
    expect(end).not.toHaveBeenCalled();

    finishPersist?.();
    await expect(sending).rejects.toMatchObject({ name: "AbortError" });
    expect(end).not.toHaveBeenCalled();

    finishTrackedWrite?.();
    await vi.advanceTimersByTimeAsync(0);
    await disposed;
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
    expect(sendMessages).not.toHaveBeenCalled();
  });

  it("skips cancellable preflight work after disposal wins lease acquisition", async () => {
    let acquireLease: (() => void) | undefined;
    const begin = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          acquireLease = resolve;
        }),
    );
    const end = vi.fn().mockResolvedValue(undefined);
    const activity = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const preflight = vi.fn();
    const sendMessages = vi.fn();
    const transport = guardChatTransport(
      {
        sendMessages,
        reconnectToStream: vi.fn().mockResolvedValue(null),
      },
      activity,
      {
        beforeSend: () => ({
          run: preflight,
          persistOnCancel: false,
        }),
      },
    );

    const sending = transport.sendMessages({
      trigger: "regenerate-message",
      chatId: "chat-1",
      messageId: undefined,
      messages: [{ id: "turn-1", role: "user", parts: [] }],
      abortSignal: new AbortController().signal,
    });
    await Promise.resolve();
    const disposed = activity.dispose();

    acquireLease?.();
    await expect(sending).rejects.toMatchObject({ name: "AbortError" });
    await vi.advanceTimersByTimeAsync(0);
    await disposed;

    expect(preflight).not.toHaveBeenCalled();
    expect(sendMessages).not.toHaveBeenCalled();
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
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

  it("bounds UI disposal without ending an unfinished native lease", async () => {
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

    expect(end).not.toHaveBeenCalled();

    controller.finish("turn-1");
    await vi.advanceTimersByTimeAsync(0);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("does not release on dispose while a finished response still has a pending write", async () => {
    let finishPersist: (() => void) | undefined;
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin: vi.fn().mockResolvedValue(undefined),
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    const attempt = await controller.start("turn-1");
    attempt?.trackCompletion(
      new Promise<void>((resolve) => {
        finishPersist = resolve;
      }),
    );
    attempt?.finish();

    const disposed = controller.dispose();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS);
    await disposed;
    expect(end).not.toHaveBeenCalled();

    finishPersist?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
  });

  it("bounds UI disposal but keeps the native lease until cleanup writes settle", async () => {
    let finishCleanup: (() => void) | undefined;
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin: vi.fn().mockResolvedValue(undefined),
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });
    await controller.start("turn-1");
    const cleanup = new Promise<void>((resolve) => {
      finishCleanup = resolve;
    });

    const disposed = controller.dispose(cleanup);
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS);
    await disposed;
    expect(end).not.toHaveBeenCalled();

    finishCleanup?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(end).toHaveBeenCalledWith("chat", "turn-1:attempt-1");
    expect(end).toHaveBeenCalledTimes(1);
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

    controller.finish("turn-1");
    await expect(disposed).resolves.toBeUndefined();
    controller.finish("turn-2");
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);

    expect(end.mock.calls).toEqual([
      ["chat", "turn-1:attempt-1"],
      ["chat", "turn-2:attempt-2"],
    ]);
  });

  it("schedules finished normal work after a resumed survivor releases", async () => {
    let acquireSurvivor: (() => void) | undefined;
    const begin = vi
      .fn()
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          acquireSurvivor = resolve;
        }),
      )
      .mockResolvedValueOnce(undefined);
    const end = vi.fn().mockResolvedValue(undefined);
    const controller = createChatCloudsyncActivityController({
      begin,
      end,
      createAttemptKey: sequentialAttemptKeys(),
    });

    await controller.dispose();
    const surviving = controller.runWithLease("survivor", () => undefined);
    await Promise.resolve();
    controller.resume();

    const normal = await controller.start("normal");
    normal?.finish();
    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS * 2);
    expect(end).not.toHaveBeenCalled();

    acquireSurvivor?.();
    await surviving;
    await vi.advanceTimersByTimeAsync(0);
    expect(end).toHaveBeenCalledWith("chat", "survivor:attempt-1");
    expect(end).not.toHaveBeenCalledWith("chat", "normal:attempt-2");

    await vi.advanceTimersByTimeAsync(CHAT_CLOUDSYNC_RELEASE_DELAY_MS);
    expect(end).toHaveBeenCalledWith("chat", "normal:attempt-2");
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
