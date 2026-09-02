const inFlightSessionIds = new Set<string>();
const pendingAutoJoinBySessionId = new Map<string, string>();

export function beginScheduledAutoStart(sessionId: string) {
  inFlightSessionIds.add(sessionId);
}

export function finishScheduledAutoStart(sessionId: string) {
  inFlightSessionIds.delete(sessionId);
}

export function hasScheduledAutoStartInFlight() {
  return inFlightSessionIds.size > 0;
}

export function queueScheduledAutoJoin(sessionId: string, meetingLink: string) {
  pendingAutoJoinBySessionId.set(sessionId, meetingLink);
}

export function takeScheduledAutoJoin(sessionId: string) {
  const meetingLink = pendingAutoJoinBySessionId.get(sessionId);
  pendingAutoJoinBySessionId.delete(sessionId);
  return meetingLink;
}
