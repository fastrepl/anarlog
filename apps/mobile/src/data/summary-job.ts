export const PENDING_SUMMARY_PREFIX = "mobile_auto_summary_pending:";

export function clearPendingSummaryStatement(sessionId: string) {
  return {
    sql: "DELETE FROM app_settings WHERE id = ?",
    params: [`${PENDING_SUMMARY_PREFIX}${sessionId}`],
  };
}
