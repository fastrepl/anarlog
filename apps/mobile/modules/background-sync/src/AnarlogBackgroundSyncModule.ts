import { NativeModule, requireOptionalNativeModule } from "expo";

declare class AnarlogBackgroundSyncModule extends NativeModule {
  setEnabled(enabled: boolean): Promise<void>;
  setPendingWork(remaining: number): Promise<void>;
  finishBackgroundFlush(): Promise<void>;
  notifySyncFailed(): Promise<void>;
}

export default requireOptionalNativeModule<AnarlogBackgroundSyncModule>(
  "AnarlogBackgroundSync",
);
