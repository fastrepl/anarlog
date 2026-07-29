import { NativeModule, registerWebModule } from "expo";

import type {
  AnarlogWatchConnectivityModuleEvents,
  PendingWatchRecording,
  WatchConnectivityState,
} from "./AnarlogWatchConnectivity.types";

class AnarlogWatchConnectivityModule extends NativeModule<AnarlogWatchConnectivityModuleEvents> {
  updateAccount(_userId: string | null, _email: string | null): void {}

  getState(): WatchConnectivityState {
    return {
      supported: false,
      activationState: "unsupported",
      paired: false,
      watchAppInstalled: false,
      reachable: false,
    };
  }

  getPendingRecordings(): PendingWatchRecording[] {
    return [];
  }

  markRecordingImported(_id: string): void {}
}

export default registerWebModule(
  AnarlogWatchConnectivityModule,
  "AnarlogWatchConnectivityModule",
);
