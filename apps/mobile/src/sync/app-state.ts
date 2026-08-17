export function shouldSyncAfterAppStateChange(
  previousState: string,
  nextState: string,
): boolean {
  return previousState !== "active" && nextState === "active";
}
