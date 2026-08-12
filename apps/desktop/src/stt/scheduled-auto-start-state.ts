const inFlightSessionIds = new Set<string>();

export function beginScheduledAutoStart(sessionId: string) {
  inFlightSessionIds.add(sessionId);
}

export function finishScheduledAutoStart(sessionId: string) {
  inFlightSessionIds.delete(sessionId);
}

export function hasScheduledAutoStartInFlight() {
  return inFlightSessionIds.size > 0;
}
