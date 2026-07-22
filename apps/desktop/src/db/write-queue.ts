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
    const uniqueKeys = [...new Set(keys)];
    let failed = false;
    let failure: unknown;
    while (true) {
      const activeKeys = uniqueKeys.filter((key) => tails.has(key));
      if (activeKeys.length === 0) {
        if (failed) throw failure;
        return;
      }
      const results = await Promise.allSettled(
        activeKeys.map(flushDatabaseWriteKey),
      );
      const rejected = results.find(
        (result): result is PromiseRejectedResult =>
          result.status === "rejected",
      );
      if (!failed && rejected) {
        failed = true;
        failure = rejected.reason;
      }
    }
  }

  while (pending.size > 0) {
    const results = await Promise.allSettled(Array.from(pending));
    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (failure) throw failure.reason;
  }
}

export async function flushDatabaseWritesByPrefix(
  prefixes: readonly string[],
): Promise<void> {
  const uniquePrefixes = [...new Set(prefixes)];
  let failed = false;
  let failure: unknown;
  while (true) {
    const keys = [...tails.keys()].filter((key) =>
      uniquePrefixes.some((prefix) => key.startsWith(prefix)),
    );
    if (keys.length === 0) {
      if (failed) throw failure;
      return;
    }
    const results = await Promise.allSettled(keys.map(flushDatabaseWriteKey));
    const rejected = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (!failed && rejected) {
      failed = true;
      failure = rejected.reason;
    }
  }
}

export async function flushDatabaseWritesWithin(
  timeoutMs: number,
  keys?: readonly string[],
): Promise<void> {
  const flush = flushDatabaseWrites(keys);
  const timeoutError = new Error("Timed out waiting for local database writes");
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    await Promise.race([
      flush,
      new Promise<never>((_resolve, reject) => {
        timeout = setTimeout(() => reject(timeoutError), timeoutMs);
      }),
    ]);
  } catch (error) {
    if (error === timeoutError) void flush.catch(() => undefined);
    throw error;
  } finally {
    if (timeout) clearTimeout(timeout);
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
