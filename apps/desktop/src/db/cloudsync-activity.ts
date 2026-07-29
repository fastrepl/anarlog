import { beginCloudsyncActivity, endCloudsyncActivity } from "@anlg/plugin-db";

export const CLOUDSYNC_ACTIVITY_END_RETRY_DELAYS_MS = [100, 300] as const;
export const CLOUDSYNC_ACTIVITY_END_RETRY_INTERVAL_MS = 5_000;

type PendingRelease = {
  handoff: Promise<void>;
  retryTimer?: ReturnType<typeof setTimeout>;
};

export function createCloudsyncActivityReleaseManager({
  end = endCloudsyncActivity,
  foregroundRetryDelaysMs = CLOUDSYNC_ACTIVITY_END_RETRY_DELAYS_MS,
  backgroundRetryIntervalMs = CLOUDSYNC_ACTIVITY_END_RETRY_INTERVAL_MS,
  onDeferred = (activity: string, key: string, error: unknown) => {
    console.error(
      `CloudSync ${activity} activity ${key} will keep retrying release`,
      error,
    );
  },
}: {
  end?: typeof endCloudsyncActivity;
  foregroundRetryDelaysMs?: readonly number[];
  backgroundRetryIntervalMs?: number;
  onDeferred?: (activity: string, key: string, error: unknown) => void;
} = {}) {
  const pending = new Map<string, PendingRelease>();
  const wait = (delayMs: number) =>
    new Promise<void>((resolve) => setTimeout(resolve, delayMs));

  const release = (activity: string, key: string): Promise<void> => {
    const releaseId = JSON.stringify([activity, key]);
    const existing = pending.get(releaseId);
    if (existing) {
      return existing.handoff;
    }

    const state: PendingRelease = {
      handoff: Promise.resolve(),
    };

    const finish = () => {
      if (state.retryTimer) {
        clearTimeout(state.retryTimer);
        state.retryTimer = undefined;
      }
      if (pending.get(releaseId) === state) {
        pending.delete(releaseId);
      }
    };

    const scheduleBackgroundRetry = () => {
      state.retryTimer = setTimeout(() => {
        state.retryTimer = undefined;
        void end(activity, key).then(finish, scheduleBackgroundRetry);
      }, backgroundRetryIntervalMs);
    };

    state.handoff = (async () => {
      let lastError: unknown;
      for (
        let attempt = 0;
        attempt <= foregroundRetryDelaysMs.length;
        attempt += 1
      ) {
        try {
          await end(activity, key);
          finish();
          return;
        } catch (error) {
          lastError = error;
          const retryDelay = foregroundRetryDelaysMs[attempt];
          if (retryDelay === undefined) {
            break;
          }
          await wait(retryDelay);
        }
      }

      try {
        onDeferred(activity, key, lastError);
      } catch (error) {
        console.error("Failed to report deferred CloudSync release", error);
      }
      scheduleBackgroundRetry();
    })();

    pending.set(releaseId, state);
    return state.handoff;
  };

  return { release };
}

const cloudsyncActivityReleaseManager = createCloudsyncActivityReleaseManager();

export const releaseCloudsyncActivityEventually =
  cloudsyncActivityReleaseManager.release;

export async function withCloudsyncActivity<T>(
  activity: string,
  key: string,
  run: () => Promise<T>,
): Promise<T> {
  await beginCloudsyncActivity(activity, key);
  try {
    return await run();
  } finally {
    await releaseCloudsyncActivityEventually(activity, key);
  }
}
