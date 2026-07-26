const DATABASE_LOCK_RETRY_DELAYS_MS = [100, 500, 1_500, 4_000];

export function isDatabaseLockError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    /database (?:table )?is locked/i.test(message) ||
    /\bcode:\s*[56]\b/i.test(message)
  );
}

export async function retryDatabaseLock<T>(
  run: () => Promise<T>,
  retryDelaysMs: readonly number[] = DATABASE_LOCK_RETRY_DELAYS_MS,
): Promise<T> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await run();
    } catch (error) {
      const retryDelay = retryDelaysMs[attempt];
      if (retryDelay === undefined || !isDatabaseLockError(error)) {
        throw error;
      }

      await new Promise<void>((resolve) => {
        setTimeout(resolve, retryDelay);
      });
    }
  }
}
