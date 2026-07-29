import type { ChatTransport, UIMessage } from "ai";

import { beginCloudsyncActivity, endCloudsyncActivity } from "@anlg/plugin-db";

export const CHAT_CLOUDSYNC_RELEASE_DELAY_MS = 750;
export const CHAT_CLOUDSYNC_DISPOSE_DRAIN_TIMEOUT_MS = 5_000;
export const CHAT_CLOUDSYNC_END_RETRY_DELAYS_MS = [100, 300] as const;
export const CHAT_CLOUDSYNC_END_RETRY_INTERVAL_MS = 5_000;

type Lease = {
  logicalKey: string;
  nativeKey: string;
  acquisition: Promise<void>;
  acquired: boolean;
  completions: Set<Promise<unknown>>;
  preflight?: GuardedChatPreflight;
  preflightPending: boolean;
  finished: boolean;
  callbackReady: boolean;
  callbackConsumed: boolean;
  finishClaimed: boolean;
  releaseOnFinish: boolean;
  surviveDisposal: boolean;
  release?: Promise<void>;
  retryTimer?: ReturnType<typeof setTimeout>;
  releaseFailureReported: boolean;
};

type Disposal = {
  leases: Lease[];
  promise: Promise<void>;
  resolve: () => void;
  timer?: ReturnType<typeof setTimeout>;
  releaseStarted: boolean;
};

export type ChatCloudsyncActivityAttempt = {
  key: string;
  finish: () => void;
  trackCompletion: (completion: Promise<unknown>) => void;
};

export type GuardedChatPreflight = {
  run: (
    trackCompletion: (completion: Promise<unknown>) => void,
  ) => void | Promise<void>;
  persistOnCancel: boolean;
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
  const pendingDisposals = new Set<Disposal>();
  let activeDisposal: Disposal | null = null;
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

  const removeFromLogicalQueue = (lease: Lease) => {
    if (
      !lease.callbackConsumed ||
      leases.has(lease.nativeKey) ||
      (lease.preflight && !lease.finishClaimed)
    ) {
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
          if (lease.releaseOnFinish) {
            scheduleReleaseIfIdle();
          }
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

  const resolveDisposal = (disposal: Disposal) => {
    if (disposal.timer) {
      clearTimeout(disposal.timer);
      disposal.timer = undefined;
    }
    disposal.resolve();
  };

  const finishDisposal = (disposal: Disposal) => {
    if (
      disposal.releaseStarted ||
      !disposal.leases.every((lease) => lease.finished)
    ) {
      return;
    }

    disposal.releaseStarted = true;
    void Promise.allSettled(disposal.leases.map(releaseLease)).then(() => {
      pendingDisposals.delete(disposal);
      resolveDisposal(disposal);
    });
  };

  const finishPendingDisposals = () => {
    for (const disposal of pendingDisposals) {
      finishDisposal(disposal);
    }
  };

  function scheduleReleaseIfIdle() {
    finishPendingDisposals();
    if (disposed) {
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
  }

  const finishLease = (lease: Lease) => {
    if (lease.finished) {
      return;
    }
    lease.finished = true;
    if (lease.releaseOnFinish) {
      finishPendingDisposals();
      if (!lease.release) {
        void releaseLease(lease).catch(() => undefined);
      }
      return;
    }
    scheduleReleaseIfIdle();
  };

  const finishLeaseIfSettled = (lease: Lease) => {
    if (
      !lease.callbackConsumed ||
      lease.preflightPending ||
      lease.completions.size > 0
    ) {
      return;
    }
    finishLease(lease);
  };

  const trackCompletion = (lease: Lease, completion: Promise<unknown>) => {
    if (!leases.has(lease.nativeKey) || lease.release) {
      return;
    }
    lease.finished = false;
    clearScheduledRelease();
    const tracked = Promise.resolve(completion)
      .catch(() => undefined)
      .finally(() => {
        lease.completions.delete(tracked);
        finishLeaseIfSettled(lease);
      });
    lease.completions.add(tracked);
  };

  const consumeCallback = (lease: Lease) => {
    if (lease.callbackConsumed) {
      return;
    }
    lease.callbackConsumed = true;
    finishLeaseIfSettled(lease);
    removeFromLogicalQueue(lease);
  };

  const claimLease = (lease: Lease | undefined) => {
    if (lease) {
      lease.finishClaimed = true;
      removeFromLogicalQueue(lease);
    }
    return lease;
  };

  const claimActiveFinishLease = (logicalKey: string) => {
    const queue = logicalQueues.get(logicalKey);
    return claimLease(
      queue?.find(
        (candidate) =>
          !candidate.finishClaimed &&
          candidate.callbackReady &&
          !candidate.callbackConsumed &&
          leases.get(candidate.nativeKey) === candidate &&
          !candidate.release &&
          !candidate.retryTimer,
      ),
    );
  };

  const claimReusableFinishLease = (logicalKey: string) => {
    const queue = logicalQueues.get(logicalKey);
    return claimLease(
      queue?.find(
        (candidate) =>
          !candidate.finishClaimed &&
          candidate.acquired &&
          candidate.callbackConsumed &&
          !candidate.preflightPending &&
          leases.get(candidate.nativeKey) === candidate &&
          !candidate.release &&
          !candidate.retryTimer,
      ),
    );
  };

  const claimRetainedPreflightLease = (
    logicalKey: string,
    excluded?: Lease,
  ) => {
    const queue = logicalQueues.get(logicalKey);
    return claimLease(
      queue?.find(
        (candidate) =>
          candidate !== excluded &&
          !candidate.finishClaimed &&
          candidate.callbackConsumed &&
          !candidate.preflightPending &&
          Boolean(candidate.preflight),
      ),
    );
  };

  const claimConsumedFinishLease = (logicalKey: string) => {
    const queue = logicalQueues.get(logicalKey);
    return claimLease(
      queue?.find(
        (candidate) =>
          !candidate.finishClaimed &&
          candidate.callbackConsumed &&
          !candidate.preflightPending,
      ),
    );
  };

  const restoreFinishLease = (lease: Lease) => {
    if (!lease.preflight) {
      return;
    }
    lease.finishClaimed = false;
    const queue = logicalQueues.get(lease.logicalKey) ?? [];
    if (!queue.includes(lease)) {
      queue.unshift(lease);
      logicalQueues.set(lease.logicalKey, queue);
    }
  };

  const discardSupersededPreflights = (successfulLease: Lease) => {
    const queue = logicalQueues.get(successfulLease.logicalKey);
    if (!queue) {
      return;
    }
    for (const lease of [...queue]) {
      if (
        lease === successfulLease ||
        lease.finishClaimed ||
        !lease.preflight ||
        (leases.get(lease.nativeKey) === lease &&
          (!lease.callbackConsumed || lease.preflightPending))
      ) {
        continue;
      }
      lease.preflight = undefined;
      removeFromLogicalQueue(lease);
    }
  };

  const runPreflight = async (sourceLease: Lease, activeLease: Lease) => {
    const preflight = sourceLease.preflight;
    if (!preflight) {
      return;
    }

    await preflight.run((completion) =>
      trackCompletion(activeLease, completion),
    );
    sourceLease.preflight = undefined;
  };

  const start = async (
    logicalKey: string,
    {
      preflight,
      isCancelled = () => false,
      surviveDisposal = false,
    }: {
      preflight?: GuardedChatPreflight;
      isCancelled?: () => boolean;
      surviveDisposal?: boolean;
    } = {},
  ): Promise<ChatCloudsyncActivityAttempt | null> => {
    if (disposed && !surviveDisposal) {
      return null;
    }

    clearScheduledRelease();
    const nativeKey = createAttemptKey(logicalKey);
    const lease: Lease = {
      logicalKey,
      nativeKey,
      acquisition: begin("chat", nativeKey),
      acquired: false,
      completions: new Set(),
      preflight,
      preflightPending: Boolean(preflight),
      finished: false,
      callbackReady: false,
      callbackConsumed: false,
      finishClaimed: false,
      releaseOnFinish: disposed && surviveDisposal,
      surviveDisposal,
      releaseFailureReported: false,
    };
    leases.set(nativeKey, lease);
    const queue = logicalQueues.get(logicalKey) ?? [];
    queue.push(lease);
    logicalQueues.set(logicalKey, queue);

    try {
      await lease.acquisition;
      lease.acquired = true;
    } catch (error) {
      lease.preflightPending = false;
      consumeCallback(lease);
      await releaseLease(lease).catch(() => undefined);
      throw error;
    }

    let preflightFailed = false;
    let preflightError: unknown;
    try {
      if (
        lease.preflight &&
        (lease.preflight.persistOnCancel || (!disposed && !isCancelled()))
      ) {
        await runPreflight(lease, lease);
        discardSupersededPreflights(lease);
      } else {
        lease.preflight = undefined;
      }
    } catch (error) {
      preflightFailed = true;
      preflightError = error;
    } finally {
      lease.preflightPending = false;
      finishLeaseIfSettled(lease);
    }

    if (preflightFailed) {
      consumeCallback(lease);
      throw preflightError;
    }

    if ((!surviveDisposal && disposed) || leases.get(nativeKey) !== lease) {
      consumeCallback(lease);
      return null;
    }

    lease.callbackReady = true;
    return {
      key: nativeKey,
      finish: () => consumeCallback(lease),
      trackCompletion: (completion) => trackCompletion(lease, completion),
    };
  };

  const finish = (logicalKey: string) => {
    const lease = claimActiveFinishLease(logicalKey);
    if (!lease) {
      return;
    }

    consumeCallback(lease);
  };

  const runWithLease = <T>(
    logicalKey: string,
    run: () => T | Promise<T>,
  ): Promise<T> => {
    const activeLease =
      claimActiveFinishLease(logicalKey) ??
      claimReusableFinishLease(logicalKey);
    if (activeLease) {
      const preflightLease = claimRetainedPreflightLease(
        logicalKey,
        activeLease,
      );
      const completion = Promise.resolve().then(async () => {
        try {
          if (preflightLease) {
            await runPreflight(preflightLease, activeLease);
          }
          await runPreflight(activeLease, activeLease);
          return await run();
        } catch (error) {
          if (preflightLease) {
            restoreFinishLease(preflightLease);
          }
          restoreFinishLease(activeLease);
          throw error;
        }
      });
      trackCompletion(activeLease, completion);
      consumeCallback(activeLease);
      return completion;
    }

    const lease =
      claimRetainedPreflightLease(logicalKey) ??
      claimConsumedFinishLease(logicalKey);
    return (async () => {
      let attempt: ChatCloudsyncActivityAttempt | null;
      try {
        attempt = await start(logicalKey, { surviveDisposal: true });
      } catch (error) {
        if (lease) {
          restoreFinishLease(lease);
        }
        throw error;
      }
      if (!attempt) {
        if (lease) {
          restoreFinishLease(lease);
        }
        const error = new Error("Chat activity unavailable");
        error.name = "AbortError";
        throw error;
      }

      const freshLease = leases.get(attempt.key);
      if (!freshLease) {
        attempt.finish();
        if (lease) {
          restoreFinishLease(lease);
        }
        throw new Error("Chat activity lease unavailable");
      }
      freshLease.finishClaimed = true;
      const completion = Promise.resolve().then(async () => {
        try {
          if (lease) {
            await runPreflight(lease, freshLease);
          }
          return await run();
        } catch (error) {
          if (lease) {
            restoreFinishLease(lease);
          }
          throw error;
        }
      });
      attempt.trackCompletion(completion);
      attempt.finish();
      return completion;
    })();
  };

  const finishAll = () => {
    for (const lease of leases.values()) {
      consumeCallback(lease);
    }
  };

  const dispose = (completion?: Promise<unknown>) => {
    if (disposed && activeDisposal) {
      return activeDisposal.promise;
    }

    disposed = true;
    clearScheduledRelease();
    let resolveDisposal = () => {};
    const promise = new Promise<void>((resolve) => {
      resolveDisposal = resolve;
    });
    const disposal: Disposal = {
      leases: [...leases.values()],
      promise,
      resolve: resolveDisposal,
      releaseStarted: false,
    };
    activeDisposal = disposal;
    pendingDisposals.add(disposal);
    for (const lease of disposal.leases) {
      if (lease.surviveDisposal) {
        lease.releaseOnFinish = true;
        continue;
      }
      if (completion) {
        trackCompletion(lease, completion);
        consumeCallback(lease);
      }
    }
    disposal.timer = setTimeout(() => {
      disposal.timer = undefined;
      disposal.resolve();
    }, disposeDrainTimeoutMs);
    finishDisposal(disposal);
    return promise;
  };

  const resume = () => {
    if (!disposed) {
      return;
    }

    disposed = false;
    activeDisposal = null;
  };

  return { start, finish, finishAll, runWithLease, dispose, resume };
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
    beforeSend?: (logicalKey: string) => GuardedChatPreflight | undefined;
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
      const preflight = beforeSend?.(key);
      let attempt: ChatCloudsyncActivityAttempt | null;
      try {
        attempt = await activity.start(key, {
          preflight,
          isCancelled: () => Boolean(options.abortSignal?.aborted),
        });
      } catch (error) {
        if (preflight?.persistOnCancel) {
          await activity
            .runWithLease(key, () => undefined)
            .catch(() => undefined);
        }
        throw error;
      }

      if (!attempt) {
        const error = new Error("Chat request aborted");
        error.name = "AbortError";
        throw error;
      }

      try {
        if (options.abortSignal?.aborted) {
          const error = new Error("Chat request aborted");
          error.name = "AbortError";
          throw error;
        }

        if (options.abortSignal?.aborted) {
          const error = new Error("Chat request aborted");
          error.name = "AbortError";
          throw error;
        }
        return await transport.sendMessages(options);
      } catch (error) {
        attempt.finish();
        throw error;
      }
    },
    reconnectToStream: (options) => transport.reconnectToStream(options),
  };
}
