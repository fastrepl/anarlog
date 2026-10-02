import { retryDatabaseLock } from "~/db/retry";

export async function persistTranscriptWrite<T>(
  write: () => Promise<T>,
  retryDelaysMs?: readonly number[],
): Promise<T> {
  return retryDatabaseLock(write, retryDelaysMs);
}

export { isDatabaseLockError } from "~/db/retry";
