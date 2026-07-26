const tails = new Map<string, Promise<unknown>>();

export function enqueueSessionAudioOperation<T>(
  sessionId: string,
  operation: () => Promise<T>,
): Promise<T> {
  const previous = tails.get(sessionId) ?? Promise.resolve();
  const next = previous.catch(() => {}).then(operation);

  tails.set(sessionId, next);
  next.then(
    () => finishOperation(sessionId, next),
    () => finishOperation(sessionId, next),
  );
  return next;
}

function finishOperation(sessionId: string, operation: Promise<unknown>) {
  if (tails.get(sessionId) === operation) {
    tails.delete(sessionId);
  }
}
