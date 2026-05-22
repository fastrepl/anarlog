export const AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX =
  "auto-stop-ended:" as const;

export function createAutoStopEndedNotificationKey(sessionId: string) {
  return `${AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX}${sessionId}`;
}

export function parseAutoStopEndedNotificationKey(
  key: string | null | undefined,
) {
  if (!key) {
    return null;
  }

  if (!key.startsWith(AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX)) {
    return null;
  }

  return key.slice(AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX.length) || null;
}
