const TRANSCRIPT_WRITE_RETRY_DELAYS_MS = [100, 500, 1_500, 4_000];

export function isDatabaseLockError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    /database (?:table )?is locked/i.test(message) ||
    /\bcode:\s*[56]\b/i.test(message)
  );
}

export async function persistTranscriptWrite(
  write: () => Promise<void>,
  retryDelaysMs = TRANSCRIPT_WRITE_RETRY_DELAYS_MS,
) {
  for (let attempt = 0; ; attempt += 1) {
    try {
      await write();
      return;
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
