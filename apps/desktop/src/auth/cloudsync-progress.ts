import { getCloudsyncStatus } from "@anlg/plugin-db";
import { commands as notificationCommands } from "@anlg/plugin-notification";

const POLL_INTERVAL_MS = 2_000;
const COMPLETED_KEY_PREFIX = "anarlog:cloudsync_initial_sync_completed:";

let monitorGeneration = 0;
let monitoredUserId: string | null = null;

function completionKey(userId: string) {
  return `${COMPLETED_KEY_PREFIX}${userId}`;
}

function readCompleted(userId: string) {
  try {
    return localStorage.getItem(completionKey(userId)) === "1";
  } catch {
    return false;
  }
}

function markCompleted(userId: string) {
  try {
    localStorage.setItem(completionKey(userId), "1");
  } catch {
    // The completion notification may repeat after restart if storage is unavailable.
  }
}

function sleep(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

async function showCompletionNotification(userId: string) {
  try {
    const result = await notificationCommands.showNotification({
      key: `cloudsync-initial-sync-complete-${userId}`,
      title: "Cloud sync complete",
      message: "Your Anarlog data is ready on this device.",
      timeout: null,
      source: null,
      start_time: null,
      participants: null,
      event_details: null,
      action_label: "Open Anarlog",
      action_variant: null,
      options: null,
      footer: null,
      icon: null,
    });

    if (result.status === "error") {
      console.error(
        "[cloudsync] failed to show completion notification",
        result.error,
      );
    }
  } catch (error) {
    console.error("[cloudsync] failed to show completion notification", error);
  }
}

async function monitorInitialSync(userId: string, activeGeneration: number) {
  while (activeGeneration === monitorGeneration) {
    try {
      const status = await getCloudsyncStatus();
      if (activeGeneration !== monitorGeneration) {
        return;
      }

      if (status.last_sync_at_ms !== null) {
        markCompleted(userId);
        monitoredUserId = null;
        await showCompletionNotification(userId);
        return;
      }

      if (
        status.configured &&
        !status.running &&
        status.last_error_kind !== null
      ) {
        monitoredUserId = null;
        return;
      }
    } catch {
      // Credential exchange and startup can briefly make status unavailable.
    }

    await sleep(POLL_INTERVAL_MS);
  }
}

export function startCloudsyncInitialSyncProgress(userId: string) {
  if (readCompleted(userId)) {
    return;
  }

  if (monitoredUserId === userId) {
    return;
  }

  const activeGeneration = ++monitorGeneration;
  monitoredUserId = userId;
  void monitorInitialSync(userId, activeGeneration);
}

export function stopCloudsyncInitialSyncProgress() {
  monitorGeneration += 1;
  monitoredUserId = null;
}
