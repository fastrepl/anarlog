export const AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX =
  "auto-stop-ended:" as const;
export const AUTO_STOP_CONFIRM_TIMEOUT_SECONDS = 30;

const AUTO_STOP_ENDED_NOTIFICATION_KEY_NONCE_SEPARATOR = ":prompt:";
const activePromptKeys = new Map<string, string>();

export function createAutoStopEndedNotificationKey(sessionId: string) {
  const activeKey = activePromptKeys.get(sessionId);
  if (activeKey) {
    return activeKey;
  }

  const key = `${AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX}${sessionId}${AUTO_STOP_ENDED_NOTIFICATION_KEY_NONCE_SEPARATOR}${crypto.randomUUID()}`;
  activePromptKeys.set(sessionId, key);
  return key;
}

export function cancelAutoStopEndedNotification(sessionId: string) {
  return activePromptKeys.delete(sessionId);
}

export function isAutoStopEndedNotificationKeyActive(key: string) {
  const sessionId = parseAutoStopEndedNotificationKey(key);
  return Boolean(sessionId && activePromptKeys.get(sessionId) === key);
}

export function consumeAutoStopEndedNotificationKey(key: string) {
  const sessionId = parseAutoStopEndedNotificationKey(key);
  if (!sessionId || !isAutoStopEndedNotificationKeyActive(key)) {
    return null;
  }

  activePromptKeys.delete(sessionId);
  return sessionId;
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
