const activeSessions = new Set<string>();
const stopHandlers = new Map<string, () => Promise<unknown>>();
const listeners = new Set<() => void>();

function notify() {
  for (const listener of listeners) listener();
}

export function beginMobileCapture(
  sessionId: string,
  stop?: () => Promise<unknown>,
): void {
  const wasActive = activeSessions.size > 0;
  activeSessions.add(sessionId);
  if (stop) stopHandlers.set(sessionId, stop);
  if (!wasActive) notify();
}

export function endMobileCapture(sessionId: string): void {
  const wasActive = activeSessions.size > 0;
  activeSessions.delete(sessionId);
  stopHandlers.delete(sessionId);
  if (wasActive && activeSessions.size === 0) notify();
}

export function subscribeMobileCapture(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getMobileCaptureActive(): boolean {
  return activeSessions.size > 0;
}

export async function stopMobileCapture(): Promise<boolean> {
  const stop = stopHandlers.values().next().value;
  if (!stop) return false;
  await stop();
  return true;
}
