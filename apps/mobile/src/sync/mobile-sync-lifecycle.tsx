import { AppState } from "react-native";

import { activateMobileAttachmentUploads } from "@/attachment-sync/upload-runner";
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
    const updateUploads = () => {
      const sync = getMobileSyncSnapshot();
      if (
        AppState.currentState === "active" &&
        sync.phase === "ready" &&
        sync.running
      ) {
        uploads.resume();
      } else {
        uploads.pause();
      }
    };
    const unsubscribeSync = subscribeMobileSync(updateUploads);
    let previousState = AppState.currentState;
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (shouldSyncAfterAppStateChange(previousState, nextState)) {
        void syncMobileNow();
      }
      previousState = nextState;
      updateUploads();
    });
    updateUploads();

    return () => {
      subscription.remove();
      unsubscribeSync();
      uploads.stop();
      deactivate();
    };
  });
  return null;
}
