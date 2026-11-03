import { AppState } from "react-native";

import { activateMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
import { enableAllPrivateAttachmentBackups } from "@/attachment-sync/upload-store";
import { retryPendingTranscriptions } from "@/data/transcribe";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";
import { activateR2DatabaseBackup } from "@/settings/r2-database-backup";
import { shouldSyncAfterAppStateChange } from "@/sync/app-state";
import { activateMobileBackgroundSync } from "@/sync/background-sync";
import {
  activateMobileSync,
  getMobileSyncSnapshot,
  subscribeMobileSync,
  syncMobileNow,
} from "@/sync/mobile-sync";

export function MobileSyncLifecycle({
  accessToken,
  accountUserId,
  syncEnabled,
}: {
  accessToken: string;
  accountUserId: string;
  syncEnabled: boolean;
}) {
  useMountEffect(() => {
    const deactivate = syncEnabled
      ? activateMobileSync({ accessToken, accountUserId })
      : () => {};
    const background = syncEnabled
      ? activateMobileBackgroundSync()
      : { refresh: () => {}, stop: () => {} };
    const uploads = activateMobileAttachmentUploads({
      accessToken,
      onActivity: background.refresh,
    });
    const databaseBackup = activateR2DatabaseBackup(accessToken);
    void enableAllPrivateAttachmentBackups()
      .then(() => uploads.resume())
      .catch((error) =>
        captureOperationalError(error, {
          operation: "enable_private_attachment_backups",
          level: "warning",
        }),
      );
    let transcriptionRetryTimer: ReturnType<typeof setInterval> | undefined;
    let syncWasReady = false;
    const retryPending = () => {
      if (
        AppState.currentState === "active" &&
        getMobileSyncSnapshot().hasRecoveryKey
      ) {
        void retryPendingTranscriptions();
      }
    };
    const updateUploads = () => {
      const sync = getMobileSyncSnapshot();
      const syncReady = sync.phase === "ready" && sync.running;
      uploads.resume();
      if (AppState.currentState === "active") databaseBackup.resume();
      else databaseBackup.pause();
      if (syncReady && !syncWasReady) retryPending();
      syncWasReady = syncReady;
      background.refresh();
    };
    const unsubscribeSync = subscribeMobileSync(updateUploads);
    let previousState = AppState.currentState;
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (shouldSyncAfterAppStateChange(previousState, nextState)) {
        void syncMobileNow();
        retryPending();
      }
      if (nextState === "active") {
        databaseBackup.resume();
      } else {
        databaseBackup.pause();
      }
      previousState = nextState;
    });
    updateUploads();
    retryPending();
    transcriptionRetryTimer = setInterval(retryPending, 60_000);

    return () => {
      if (transcriptionRetryTimer) clearInterval(transcriptionRetryTimer);
      subscription.remove();
      unsubscribeSync();
      uploads.stop();
      databaseBackup.stop();
      background.stop();
      deactivate();
    };
  });
  return null;
}
