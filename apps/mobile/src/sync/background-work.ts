import type { MobileSyncSnapshot } from "./controller";

export function backgroundSyncWork(
  sync: Pick<
    MobileSyncSnapshot,
    | "phase"
    | "running"
    | "syncingNow"
    | "hasUnsentChanges"
    | "consecutiveFailures"
  >,
  pendingUploads: number,
): number {
  if (sync.phase !== "ready" || !sync.running) return 0;
  const syncPending =
    sync.syncingNow ||
    (sync.hasUnsentChanges === true && sync.consecutiveFailures === 0);
  return Math.max(0, pendingUploads) + (syncPending ? 1 : 0);
}

export function backgroundSyncFailed(
  sync: Pick<
    MobileSyncSnapshot,
    "phase" | "hasUnsentChanges" | "errorMessage" | "consecutiveFailures"
  >,
  failedUploads: number,
): boolean {
  if (sync.phase !== "ready") return false;
  if (failedUploads > 0) return true;
  return (
    sync.hasUnsentChanges !== false &&
    (sync.consecutiveFailures > 0 || sync.errorMessage !== null)
  );
}
