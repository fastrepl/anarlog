import { AppState } from "react-native";

import { requestMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
import {
  countDueMobileAttachmentUploads,
  countFailedMobileAttachmentUploads,
} from "@/attachment-sync/upload-store";
import { captureOperationalError } from "@/lib/error-reporting";
import { nowIso } from "@/lib/ids";

import BackgroundSyncModule from "../../modules/background-sync";
import { backgroundSyncFailed, backgroundSyncWork } from "./background-work";
import { getMobileSyncSnapshot, syncMobileNow } from "./mobile-sync";

export function activateMobileBackgroundSync(): {
  refresh: () => void;
  stop: () => void;
} {
  const native = BackgroundSyncModule;
  if (!native) return { refresh: () => {}, stop: () => {} };

  let stopped = false;
  let lastReported = "";
  let backgroundSince: string | null = null;
  let flushing = false;
  let notified = false;

  const report = (action: string, work: Promise<void>) => {
    work.catch((error: unknown) =>
      captureOperationalError(error, {
        operation: `background_sync_${action}`,
        level: "warning",
      }),
    );
  };

  const update = async () => {
    const since = backgroundSince;
    const [uploads, failedUploads, newFailedUploads] = await Promise.all([
      countDueMobileAttachmentUploads(),
      countFailedMobileAttachmentUploads(),
      since === null ? 0 : countFailedMobileAttachmentUploads(since),
    ]);
    if (stopped) return;
    const snapshot = getMobileSyncSnapshot();
    const remaining = backgroundSyncWork(snapshot, uploads);
    const synced =
      remaining === 0 &&
      snapshot.hasUnsentChanges === false &&
      failedUploads === 0;
    if (
      since !== null &&
      since === backgroundSince &&
      !flushing &&
      !notified &&
      backgroundSyncFailed(snapshot, newFailedUploads)
    ) {
      notified = true;
      report("notify_failure", native.notifySyncFailed());
    }
    const key = `${remaining}:${synced}`;
    if (key === lastReported) return;
    lastReported = key;
    await native.setPendingWork(remaining, synced);
  };

  const refresh = () => {
    if (!stopped) report("refresh", update());
  };

  const flush = async () => {
    backgroundSince = nowIso();
    notified = false;
    flushing = true;
    requestMobileAttachmentUploads();
    try {
      await syncMobileNow();
    } finally {
      flushing = false;
      if (!stopped) {
        await update().catch((error: unknown) =>
          captureOperationalError(error, {
            operation: "background_sync_refresh",
            level: "warning",
          }),
        );
        await native.finishBackgroundFlush();
      }
    }
  };

  report("enable", native.setEnabled(true));
  const subscription = AppState.addEventListener("change", (nextState) => {
    if (nextState === "background") report("flush", flush());
    if (nextState === "active") backgroundSince = null;
  });
  refresh();

  return {
    refresh,
    stop: () => {
      stopped = true;
      subscription.remove();
      report("disable", native.setEnabled(false));
    },
  };
}
