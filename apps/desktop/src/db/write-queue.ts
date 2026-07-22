const tails = new Map<string, Promise<unknown>>();
const pending = new Set<Promise<unknown>>();

export function enqueueDatabaseWrite<T>(
  key: string,
  write: () => Promise<T>,
): Promise<T> {
  const previous = tails.get(key) ?? Promise.resolve();
  const next = previous.catch(() => {}).then(write);

  tails.set(key, next);
  pending.add(next);
  next.then(
    () => finishWrite(key, next),
    () => finishWrite(key, next),
  );

  return next;
}

export async function flushDatabaseWrites(
  keys?: readonly string[],
): Promise<void> {
  if (keys) {
    await Promise.all([...new Set(keys)].map(flushDatabaseWriteKey));
    return;
  }

  while (pending.size > 0) {
    const results = await Promise.allSettled(Array.from(pending));
    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (failure) throw failure.reason;
  }
}

async function flushDatabaseWriteKey(key: string): Promise<void> {
  let failed = false;
  let failure: unknown;
  while (true) {
    const write = tails.get(key);
    if (!write) {
      if (failed) throw failure;
      return;
    }
    try {
      await write;
    } catch (error) {
      if (!failed) failure = error;
      failed = true;
    }
  }
}

function finishWrite(key: string, write: Promise<unknown>) {
  pending.delete(write);
  if (tails.get(key) === write) tails.delete(key);
}
