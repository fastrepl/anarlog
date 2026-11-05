export function throwIfAborted(signal?: AbortSignal | null): void {
  if (!signal?.aborted) return;
  // React Native's AbortSignal has neither throwIfAborted() nor an abort reason.
  throw (
    signal.reason ??
    Object.assign(new Error("Operation cancelled."), { name: "AbortError" })
  );
}
