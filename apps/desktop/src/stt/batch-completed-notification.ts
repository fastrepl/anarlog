const LEGACY_BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX = "batch-completed-";

export const BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX =
  "batch-completed:" as const;

export function createBatchCompletedNotificationKey(sessionId: string) {
  return `${BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX}${sessionId}:${crypto.randomUUID()}`;
}

export function parseBatchCompletedNotificationKey(
  key: string | null | undefined,
) {
  if (!key) {
    return null;
  }

  if (key.startsWith(BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX)) {
    const value = key.slice(BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX.length);
    const separatorIndex = value.lastIndexOf(":");
    return (
      value.slice(0, separatorIndex === -1 ? undefined : separatorIndex) || null
    );
  }

  if (key.startsWith(LEGACY_BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX)) {
    return (
      key.slice(LEGACY_BATCH_COMPLETED_NOTIFICATION_KEY_PREFIX.length) || null
    );
  }

  return null;
}
