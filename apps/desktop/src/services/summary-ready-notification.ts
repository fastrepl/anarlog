const SUMMARY_READY_NOTIFICATION_KEY_PREFIX = "summary-ready:";

export function createSummaryReadyNotificationKey(
  sessionId: string,
  enhancedNoteId: string,
) {
  return `${SUMMARY_READY_NOTIFICATION_KEY_PREFIX}${sessionId}:${enhancedNoteId}`;
}

export function parseSummaryReadyNotificationKey(key: string) {
  if (!key.startsWith(SUMMARY_READY_NOTIFICATION_KEY_PREFIX)) {
    return null;
  }

  const payload = key.slice(SUMMARY_READY_NOTIFICATION_KEY_PREFIX.length);
  const separatorIndex = payload.indexOf(":");
  if (separatorIndex === -1) {
    return null;
  }

  const sessionId = payload.slice(0, separatorIndex);
  const enhancedNoteId = payload.slice(separatorIndex + 1);

  if (!sessionId || !enhancedNoteId) {
    return null;
  }

  return { sessionId, enhancedNoteId };
}
