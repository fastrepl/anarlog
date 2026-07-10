const tails = new Map<string, Promise<void>>();
const pending = new Set<Promise<void>>();

export function enqueueDatabaseWrite(
  key: string,
  write: () => Promise<void>,
): Promise<void> {
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

export async function flushDatabaseWrites(): Promise<void> {
  while (pending.size > 0) {
    const results = await Promise.allSettled(Array.from(pending));
    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (failure) throw failure.reason;
  }
}

function finishWrite(key: string, write: Promise<void>) {
  pending.delete(write);
  if (tails.get(key) === write) tails.delete(key);
}
