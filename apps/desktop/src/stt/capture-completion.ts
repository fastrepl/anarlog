import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

const STOP_PREFIX = "capture_stop:";

export type CaptureUsage = {
  startedAtMs: number;
  requestedLiveTranscription: boolean;
  liveTranscriptionActiveAtStop: boolean;
};

// Keep the native stop clock across retries and app restarts. A recovery attempt
// must not replace the recording's end with the time transcription was repaired.
export function saveCaptureStop(
  sessionId: string,
  transcriptId: string,
  stoppedAtMs: number,
  usage?: CaptureUsage,
): Promise<void> {
  if (!Number.isSafeInteger(stoppedAtMs) || stoppedAtMs <= 0) {
    return Promise.reject(new Error("Invalid capture stop timestamp"));
  }
  if (
    usage &&
    (!Number.isSafeInteger(usage.startedAtMs) ||
      usage.startedAtMs < 0 ||
      usage.startedAtMs >= stoppedAtMs)
  ) {
    return Promise.reject(new Error("Invalid capture interval"));
  }
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const statements = [
      {
        sql: `INSERT INTO app_settings (id, value_json, updated_at)
          VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
          ON CONFLICT(id) DO NOTHING`,
        params: [
          `${STOP_PREFIX}${sessionId}:${transcriptId}`,
          JSON.stringify(stoppedAtMs),
        ],
      },
    ];
    if (usage) {
      statements.push({
        sql: `INSERT INTO app_settings (id, value_json, updated_at)
          SELECT ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE EXISTS (SELECT 1 FROM managed_app_settings
            WHERE id = 'intelligence_disabled' AND value_json = 'true')
            AND EXISTS (SELECT 1 FROM app_settings WHERE id = ? AND value_json = ?)
          ON CONFLICT(id) DO NOTHING`,
        params: [
          `capture_usage:${sessionId}:${transcriptId}`,
          JSON.stringify({
            version: 1,
            sessionId,
            transcriptId,
            stoppedAtMs,
            ...usage,
          }),
          `${STOP_PREFIX}${sessionId}:${transcriptId}`,
          JSON.stringify(stoppedAtMs),
        ],
      });
    }
    await executeTransaction(statements);
  });
}

export async function loadCaptureStop(
  sessionId: string,
  transcriptId: string,
): Promise<number | null> {
  const rows = await liveQueryClient.execute<{ value_json: string }>(
    "SELECT value_json FROM app_settings WHERE id = ?",
    [`${STOP_PREFIX}${sessionId}:${transcriptId}`],
  );
  const value = Number(rows[0]?.value_json);
  return Number.isSafeInteger(value) && value > 0 ? value : null;
}

// Call only after successful transcript persistence/repair. The pending lifecycle
// marker still excludes this session from managed export until finalization ends.
export function completeCaptureTranscript(
  sessionId: string,
  transcriptId: string,
  stoppedAtMs: number,
): Promise<void> {
  if (!Number.isSafeInteger(stoppedAtMs) || stoppedAtMs <= 0) {
    return Promise.reject(new Error("Invalid capture stop timestamp"));
  }
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    await executeTransaction([
      {
        sql: `UPDATE transcripts
          SET ended_at_ms = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ? AND session_id = ? AND deleted_at IS NULL
            AND started_at_ms <= ?
            AND EXISTS (SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)`,
        params: [stoppedAtMs, transcriptId, sessionId, stoppedAtMs, sessionId],
      },
      {
        sql: `UPDATE sessions
          SET started_at = CASE WHEN started_at = '' THEN
                strftime('%Y-%m-%dT%H:%M:%fZ', (
                  SELECT MIN(started_at_ms) FROM transcripts
                  WHERE session_id = ? AND deleted_at IS NULL
                ) / 1000.0, 'unixepoch') ELSE started_at END,
              ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', (
                SELECT MAX(ended_at_ms) FROM transcripts
                WHERE session_id = ? AND deleted_at IS NULL
              ) / 1000.0, 'unixepoch'),
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ? AND deleted_at IS NULL
            AND EXISTS (SELECT 1 FROM transcripts
              WHERE id = ? AND session_id = ? AND deleted_at IS NULL
                AND ended_at_ms = ?)`,
        params: [
          sessionId,
          sessionId,
          sessionId,
          transcriptId,
          sessionId,
          stoppedAtMs,
        ],
      },
    ]);
  });
}
