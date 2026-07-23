import { retryDatabaseLock } from "~/db/retry";

export async function persistTranscriptWrite(
  write: () => Promise<void>,
  retryDelaysMs?: readonly number[],
) {
  await retryDatabaseLock(write, retryDelaysMs);
}

export { isDatabaseLockError } from "~/db/retry";
