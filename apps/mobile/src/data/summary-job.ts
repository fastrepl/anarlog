import { id, nowIso } from "@/lib/ids";

export const PENDING_SUMMARY_PREFIX = "mobile_auto_summary_pending:";

export function pendingSummaryStatement(sessionId: string) {
  return {
    sql: `INSERT INTO app_settings (id, value_json, updated_at)
      SELECT ?, ?, ? FROM sessions WHERE id = ? AND deleted_at IS NULL
        AND EXISTS (SELECT 1 FROM session_attachments
          WHERE session_id = sessions.id AND source_type = 'session_audio' AND deleted_at IS NULL
            AND json_extract(metadata_json, '$.transcript_status') = 'complete')
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at`,
    params: [
      `${PENDING_SUMMARY_PREFIX}${sessionId}`,
      JSON.stringify({ generation: id() }),
      nowIso(),
      sessionId,
    ],
  };
}
