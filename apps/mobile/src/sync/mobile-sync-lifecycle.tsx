import { AppState } from "react-native";

import { activateMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
import { retryPendingTranscriptions } from "@/data/transcribe";
import { useMountEffect } from "@/lib/use-mount-effect";
import { shouldSyncAfterAppStateChange } from "@/sync/app-state";
import {
  activateMobileSync,
  getMobileSyncSnapshot,
  subscribeMobileSync,
  syncMobileNow,
} from "@/sync/mobile-sync";

export function MobileSyncLifecycle({
  accessToken,
  accountUserId,
}: {
  accessToken: string;
  accountUserId: string;
}) {
  useMountEffect(() => {
    const deactivate = activateMobileSync({ accessToken, accountUserId });
    const uploads = activateMobileAttachmentUploads({ accessToken });
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
      if (syncReady) {
        uploads.resume();
      } else {
        uploads.pause();
      }
      if (syncReady && !syncWasReady) retryPending();
      syncWasReady = syncReady;
    };
    const unsubscribeSync = subscribeMobileSync(updateUploads);
    let previousState = AppState.currentState;
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (shouldSyncAfterAppStateChange(previousState, nextState)) {
        void syncMobileNow();
        retryPending();
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
      deactivate();
    };
  });
  return null;
}
