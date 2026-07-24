import type { ChatTransport, UIMessage } from "ai";

import { beginCloudsyncActivity, endCloudsyncActivity } from "@hypr/plugin-db";

export const CHAT_CLOUDSYNC_RELEASE_DELAY_MS = 750;
export const CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS = 5_000;
export const CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS = [100, 300] as const;
export const CHAT_CLOUDSYNC_END_RETRY_INTERVAL_MS = 5_000;

type Lease = {
  logicalKey: string;
  nativeKey: string;
  acquisition: Promise<void>;
  finished: boolean;
  callbackConsumed: boolean;
  release?: Promise<void>;
  retryTimer?: ReturnType<typeof setTimeout>;
  releaseFailureReported: boolean;
};

export type ChatCloudsyncActivityAttempt = {
  key: string;
  finish: () => void;
};

export function createChatCloudsyncActivityController({
  begin = beginCloudsyncActivity,
  end = endCloudsyncActivity,
  releaseDelayMs = CHAT_CLOUDSYNC_RELEASE_DELAY_MS,
  disposeDrainTimeoutMs = CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS,
  endRetryDelaysMs = CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS,
  endRetryIntervalMs = CHAT_CLOUDSYNC_END_RETRY_INTERVAL_MS,
  createAttemptKey = (logicalKey: string) =>
    `${logicalKey}:${crypto.randomUUID()}`,
  onReleaseError = (error: unknown) => {
    console.error("Failed to resume cloud sync after chat activity", error);
  },
}: {
  begin?: typeof beginCloudsyncActivity;
  end?: typeof endCloudsyncActivity;
  releaseDelayMs?: number;
  disposeDrainTimeoutMs?: number;
  endRetryDelaysMs?: readonly number[];
  endRetryIntervalMs?: number;
  createAttemptKey?: (logicalKey: string) => string;
  onReleaseError?: (error: unknown) => void;
} = {}) {
  const leases = new Map<string, Lease>();
  const logicalQueues = new Map<string, Lease[]>();
  let releaseTimer: ReturnType<typeof setTimeout> | null = null;
  let disposeTimer: ReturnType<typeof setTimeout> | null = null;
  let disposePromise: Promise<void> | null = null;
  let resolveDispose: (() => void) | null = null;
  let disposeReleaseStarted = false;
  let lifecycleGeneration = 0;
  let disposed = false;

  const wait = (delayMs: number) =>
    new Promise<void>((resolve) => setTimeout(resolve, delayMs));

  const clearScheduledRelease = () => {
    if (releaseTimer === null) {
      return;
    }
    clearTimeout(releaseTimer);
    releaseTimer = null;
  };

  const clearDisposeTimer = () => {
    if (disposeTimer === null) {
      return;
    }
    clearTimeout(disposeTimer);
    disposeTimer = null;
  };

  const removeFromLogicalQueue = (lease: Lease) => {
    if (!lease.callbackConsumed || leases.has(lease.nativeKey)) {
      return;
    }
    const queue = logicalQueues.get(lease.logicalKey);
    if (!queue) {
      return;
    }
    const index = queue.indexOf(lease);
    if (index !== -1) {
      queue.splice(index, 1);
    }
    if (queue.length === 0) {
      logicalQueues.delete(lease.logicalKey);
    }
  };

  const releaseLease = (lease: Lease) => {
    if (lease.release) {
      return lease.release;
    }
    if (lease.retryTimer) {
      clearTimeout(lease.retryTimer);
      lease.retryTimer = undefined;
    }

    const pendingRelease = (async () => {
      await lease.acquisition.catch(() => undefined);
      let lastError: unknown;
      for (let attempt = 0; attempt <= endRetryDelaysMs.length; attempt++) {
        try {
          await end("chat", lease.nativeKey);
          if (leases.get(lease.nativeKey) === lease) {
            leases.delete(lease.nativeKey);
          }
          if (lease.retryTimer) {
            clearTimeout(lease.retryTimer);
            lease.retryTimer = undefined;
          }
          removeFromLogicalQueue(lease);
          return;
        } catch (error) {
          lastError = error;
          const retryDelay = endRetryDelaysMs[attempt];
          if (retryDelay === undefined) {
            break;
          }
          await wait(retryDelay);
        }
      }

      if (!lease.releaseFailureReported) {
        lease.releaseFailureReported = true;
        try {
          onReleaseError(lastError);
        } catch (reportError) {
          console.error(
            "Failed to report chat CloudSync release error",
            reportError,
          );
        }
      }
      lease.retryTimer = setTimeout(() => {
        lease.retryTimer = undefined;
        void releaseLease(lease).catch(() => undefined);
      }, endRetryIntervalMs);
      throw lastError;
    })().finally(() => {
      if (lease.release === pendingRelease) {
        lease.release = undefined;
      }
    });
    lease.release = pendingRelease;
    return pendingRelease;
  };

  const releaseAll = async () => {
    clearScheduledRelease();
    await Promise.allSettled([...leases.values()].map(releaseLease));
  };

  const allLeasesFinished = () =>
    [...leases.values()].every((lease) => lease.finished);

  const finishDisposedController = (force: boolean) => {
    if (
      !disposed ||
      disposeReleaseStarted ||
      (!force && !allLeasesFinished())
    ) {
      return;
    }

    const generation = lifecycleGeneration;
    disposeReleaseStarted = true;
    clearDisposeTimer();
    void releaseAll().finally(() => {
      if (generation !== lifecycleGeneration) {
        return;
      }
      logicalQueues.clear();
      resolveDispose?.();
      resolveDispose = null;
    });
  };

  const scheduleReleaseIfIdle = () => {
    if (disposed) {
      finishDisposedController(false);
      return;
    }
    if (leases.size === 0 || !allLeasesFinished()) {
      return;
    }

    clearScheduledRelease();
    releaseTimer = setTimeout(() => {
      releaseTimer = null;
      void releaseAll();
    }, releaseDelayMs);
  };

  const finishLease = (lease: Lease) => {
    if (lease.finished) {
      return;
    }
    lease.finished = true;
    scheduleReleaseIfIdle();
  };

  const start = async (
    logicalKey: string,
  ): Promise<ChatCloudsyncActivityAttempt | null> => {
    if (disposed) {
      return null;
    }

    clearScheduledRelease();
    const nativeKey = createAttemptKey(logicalKey);
    const lease: Lease = {
      logicalKey,
      nativeKey,
      acquisition: begin("chat", nativeKey),
      finished: false,
      callbackConsumed: false,
      releaseFailureReported: false,
    };
    leases.set(nativeKey, lease);
    const queue = logicalQueues.get(logicalKey) ?? [];
    queue.push(lease);
    logicalQueues.set(logicalKey, queue);

    try {
      await lease.acquisition;
    } catch (error) {
      finishLease(lease);
      await releaseLease(lease).catch(() => undefined);
      throw error;
    }

    if (disposed || leases.get(nativeKey) !== lease) {
      finishLease(lease);
      return null;
    }

    return {
      key: nativeKey,
      finish: () => consumeCallback(lease),
    };
  };

  const consumeCallback = (lease: Lease) => {
    if (lease.callbackConsumed) {
      return;
    }
    lease.callbackConsumed = true;
    finishLease(lease);
    removeFromLogicalQueue(lease);
  };

  const finish = (logicalKey: string) => {
    const queue = logicalQueues.get(logicalKey);
    const lease = queue?.find((candidate) => !candidate.callbackConsumed);
    if (!lease) {
      return;
    }

    consumeCallback(lease);
  };

  const finishAll = () => {
    for (const lease of leases.values()) {
      lease.finished = true;
    }
    scheduleReleaseIfIdle();
  };

  const dispose = () => {
    if (disposePromise) {
      return disposePromise;
    }

    disposed = true;
    clearScheduledRelease();
    const generation = ++lifecycleGeneration;
    disposePromise = new Promise<void>((resolve) => {
      resolveDispose = resolve;
    });
    disposeTimer = setTimeout(() => {
      if (generation === lifecycleGeneration) {
        finishDisposedController(true);
      }
    }, disposeDrainTimeoutMs);
    finishDisposedController(false);
    return disposePromise;
  };

  const resume = () => {
    if (!disposed) {
      return;
    }

    lifecycleGeneration += 1;
    clearDisposeTimer();
    disposed = false;
    disposeReleaseStarted = false;
    resolveDispose?.();
    resolveDispose = null;
    disposePromise = null;
  };

  return { start, finish, finishAll, dispose, resume };
}

export type ChatCloudsyncActivityController = ReturnType<
  typeof createChatCloudsyncActivityController
>;

export function guardChatTransport<UI_MESSAGE extends UIMessage>(
  transport: ChatTransport<UI_MESSAGE>,
  activity: ChatCloudsyncActivityController,
  {
    beforeSend,
  }: {
    beforeSend?: (
      logicalKey: string,
    ) => (() => void | Promise<void>) | undefined;
  } = {},
): ChatTransport<UI_MESSAGE> {
  return {
    sendMessages: async (options) => {
      let userMessage: UI_MESSAGE | undefined;
      for (let i = options.messages.length - 1; i >= 0; i--) {
        if (options.messages[i].role === "user") {
          userMessage = options.messages[i];
          break;
        }
      }
      if (!userMessage) {
        throw new Error("Cannot start chat activity without a user message");
      }

      const key = userMessage.id;
      const attempt = await activity.start(key);

      if (!attempt || options.abortSignal?.aborted) {
        const error = new Error("Chat request aborted");
        error.name = "AbortError";
        throw error;
      }

      await beforeSend?.(key)?.();
      if (options.abortSignal?.aborted) {
        const error = new Error("Chat request aborted");
        error.name = "AbortError";
        throw error;
      }
      return transport.sendMessages(options);
    },
    reconnectToStream: (options) => transport.reconnectToStream(options),
  };
}
