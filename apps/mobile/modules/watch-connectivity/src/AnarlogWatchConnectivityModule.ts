import { NativeModule, requireOptionalNativeModule } from "expo";

import type {
  AnarlogWatchConnectivityModuleEvents,
  PendingWatchRecording,
  WatchConnectivityState,
} from "./AnarlogWatchConnectivity.types";

declare class AnarlogWatchConnectivityModule extends NativeModule<AnarlogWatchConnectivityModuleEvents> {
  updateAccount(userId: string | null, email: string | null): void;
  getState(): WatchConnectivityState;
  getPendingRecordings(): PendingWatchRecording[];
  markRecordingImported(id: string): void;
}

export default requireOptionalNativeModule<AnarlogWatchConnectivityModule>(
  "AnarlogWatchConnectivity",
);
