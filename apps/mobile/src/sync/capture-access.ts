import type { MobileSyncSnapshot } from "./controller";

export function canUseMobileCapture(
  sync: Pick<MobileSyncSnapshot, "phase" | "accountUserId" | "hasRecoveryKey">,
  accountUserId: string,
): boolean {
  if (!sync.hasRecoveryKey || sync.accountUserId !== accountUserId) {
    return false;
  }
  return (
    sync.phase === "inactive" ||
    sync.phase === "starting" ||
    sync.phase === "ready" ||
    sync.phase === "error"
  );
}
