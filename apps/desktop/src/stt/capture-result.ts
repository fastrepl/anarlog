import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

export function saveIncompleteCapture(
  sessionId: string,
  transcriptId: string,
  audioDeleted: boolean,
  audioDeletionFailed?: boolean,
) {
  return enqueueDatabaseWrite(`session:${sessionId}`, () =>
    executeTransaction([
      {
        sql: `INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET value_json = json_patch(
        CASE WHEN json_valid(app_settings.value_json) THEN app_settings.value_json ELSE '{}' END,
        excluded.value_json
      ), updated_at = excluded.updated_at`,
        params: [
          `capture_incomplete:${sessionId}:${transcriptId}`,
          JSON.stringify({ audioDeleted, audioDeletionFailed }),
          new Date().toISOString(),
        ],
      },
    ]).then(() => undefined),
  );
}

export function useIncompleteCapture(sessionId: string) {
  const { data } = useLiveQuery<
    { audio_deleted: number; audio_deletion_failed: number },
    { audioDeleted: boolean; audioDeletionFailed: boolean } | null
  >({
    sql: `SELECT json_extract(value_json, '$.audioDeleted') AS audio_deleted,
      json_extract(value_json, '$.audioDeletionFailed') AS audio_deletion_failed
      FROM app_settings WHERE substr(id, 1, length(?)) = ? AND json_valid(value_json)
      AND (json_extract(value_json, '$.audioDeletionFailed') = 1 OR EXISTS (
        SELECT 1 FROM transcripts WHERE session_id = ? AND deleted_at IS NULL
        AND app_settings.id = ? || transcripts.id
      )) ORDER BY audio_deletion_failed DESC, audio_deleted DESC LIMIT 1`,
    params: [
      `capture_incomplete:${sessionId}:`,
      `capture_incomplete:${sessionId}:`,
      sessionId,
      `capture_incomplete:${sessionId}:`,
    ],
    mapRows: (rows) =>
      rows.length
        ? {
            audioDeleted: Boolean(rows[0]?.audio_deleted),
            audioDeletionFailed: Boolean(rows[0]?.audio_deletion_failed),
          }
        : null,
  });
  return data;
}

export function clearIncompleteCapture(
  sessionId: string,
  transcriptId?: string,
) {
  const prefix = `capture_incomplete:${sessionId}:`;
  return enqueueDatabaseWrite(`session:${sessionId}`, () =>
    executeTransaction([
      {
        // Text repair does not prove that private audio was deleted.
        sql: `DELETE FROM app_settings WHERE ${transcriptId ? "id = ?" : "substr(id, 1, length(?)) = ?"}
          AND COALESCE(json_extract(CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END,
            '$.audioDeletionFailed'), 0) != 1`,
        params: transcriptId ? [`${prefix}${transcriptId}`] : [prefix, prefix],
      },
    ]).then(() => undefined),
  );
}
