export function runWhenIdle(
  callback: () => void,
  {
    timeout = 5000,
    fallbackDelay = 2000,
  }: { timeout?: number; fallbackDelay?: number } = {},
): () => void {
  if (typeof window.requestIdleCallback === "function") {
    const idleId = window.requestIdleCallback(callback, { timeout });
    return () => window.cancelIdleCallback(idleId);
  }

  const timeoutId = window.setTimeout(callback, fallbackDelay);
  return () => window.clearTimeout(timeoutId);
}
