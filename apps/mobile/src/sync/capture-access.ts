import type { MobileSyncSnapshot } from "./controller";

export function canUseMobileCapture(
  sync: Pick<MobileSyncSnapshot, "phase" | "hasRecoveryKey">,
): boolean {
  if (sync.phase === "ready") return sync.hasRecoveryKey;
  return (
    sync.hasRecoveryKey && (sync.phase === "starting" || sync.phase === "error")
  );
}
