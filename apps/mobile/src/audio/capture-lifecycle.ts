const activeSessions = new Set<string>();
const listeners = new Set<() => void>();

function notify() {
  for (const listener of listeners) listener();
}

export function beginMobileCapture(sessionId: string): void {
  const wasActive = activeSessions.size > 0;
  activeSessions.add(sessionId);
  if (!wasActive) notify();
}

export function endMobileCapture(sessionId: string): void {
  const wasActive = activeSessions.size > 0;
  activeSessions.delete(sessionId);
  if (wasActive && activeSessions.size === 0) notify();
}

export function subscribeMobileCapture(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getMobileCaptureActive(): boolean {
  return activeSessions.size > 0;
}
