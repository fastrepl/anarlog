import { AppState } from "react-native";

import { requestMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
import { countDueMobileAttachmentUploads } from "@/attachment-sync/upload-store";
import { captureOperationalError } from "@/lib/error-reporting";

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
  let lastReported: number | undefined;

  const report = (action: string, work: Promise<void>) => {
    work.catch((error: unknown) =>
      captureOperationalError(error, {
        operation: `background_sync_${action}`,
        level: "warning",
      }),
    );
  };

  const refresh = () => {
    if (stopped) return;
    report(
      "refresh",
      countDueMobileAttachmentUploads().then(async (uploads) => {
        if (stopped) return;
        const remaining = backgroundSyncWork(getMobileSyncSnapshot(), uploads);
        if (remaining === lastReported) return;
        lastReported = remaining;
        await native.setPendingWork(remaining);
      }),
    );
  };

  const flush = async () => {
    requestMobileAttachmentUploads();
    try {
      await syncMobileNow();
    } finally {
      refresh();
      if (!stopped) {
        if (backgroundSyncFailed(getMobileSyncSnapshot())) {
          report("notify_failure", native.notifySyncFailed());
        }
        await native.finishBackgroundFlush();
      }
    }
  };

  report("enable", native.setEnabled(true));
  const subscription = AppState.addEventListener("change", (nextState) => {
    if (nextState === "background") report("flush", flush());
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
