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

  const notifyIfFailed = async () => {
    const since = backgroundSince;
    if (since === null || flushing || notified) return;
    const failedUploads = await countFailedMobileAttachmentUploads(since);
    if (stopped || backgroundSince !== since || notified) return;
    if (!backgroundSyncFailed(getMobileSyncSnapshot(), failedUploads)) return;
    notified = true;
    await native.notifySyncFailed();
  };

  const refresh = () => {
    if (stopped) return;
    report(
      "refresh",
      Promise.all([
        countDueMobileAttachmentUploads(),
        countFailedMobileAttachmentUploads(),
      ]).then(async ([uploads, failedUploads]) => {
        if (stopped) return;
        const snapshot = getMobileSyncSnapshot();
        const remaining = backgroundSyncWork(snapshot, uploads);
        const synced =
          remaining === 0 &&
          snapshot.hasUnsentChanges === false &&
          failedUploads === 0;
        const key = `${remaining}:${synced}`;
        if (key !== lastReported) {
          lastReported = key;
          await native.setPendingWork(remaining, synced);
        }
        await notifyIfFailed();
      }),
    );
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
      refresh();
      if (!stopped) await native.finishBackgroundFlush();
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
