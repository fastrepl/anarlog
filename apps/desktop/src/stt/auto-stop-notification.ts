export const AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX =
  "auto-stop-ended:" as const;

const AUTO_STOP_ENDED_NOTIFICATION_KEY_NONCE_SEPARATOR = ":prompt:";

export function createAutoStopEndedNotificationKey(sessionId: string) {
  return `${AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX}${sessionId}${AUTO_STOP_ENDED_NOTIFICATION_KEY_NONCE_SEPARATOR}${crypto.randomUUID()}`;
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

  const value = key.slice(AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX.length);
  const separatorIndex = value.lastIndexOf(
    AUTO_STOP_ENDED_NOTIFICATION_KEY_NONCE_SEPARATOR,
  );

  return (
    value.slice(0, separatorIndex === -1 ? undefined : separatorIndex) || null
  );
}
