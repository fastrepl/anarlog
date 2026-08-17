import { AppState } from "react-native";

import { useMountEffect } from "@/lib/use-mount-effect";
import { shouldSyncAfterAppStateChange } from "@/sync/app-state";
import { activateMobileSync, syncMobileNow } from "@/sync/mobile-sync";

export function MobileSyncLifecycle({
  accessToken,
  accountUserId,
}: {
  accessToken: string;
  accountUserId: string;
}) {
  useMountEffect(() => {
    const deactivate = activateMobileSync({ accessToken, accountUserId });
    let previousState = AppState.currentState;
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (shouldSyncAfterAppStateChange(previousState, nextState)) {
        void syncMobileNow();
      }
      previousState = nextState;
    });

    return () => {
      subscription.remove();
      deactivate();
    };
  });
  return null;
}
