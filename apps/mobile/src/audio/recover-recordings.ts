import { File, FileMode, Paths } from "expo-file-system";

import { isMobileSessionCaptureActive } from "@/audio/capture-lifecycle";
import { recoverableWav, WAV_HEADER_BYTES, wavHeader } from "@/audio/pcm-wav";
import { catalogSessionAudio, clearPendingCapture } from "@/data/audio-catalog";
import { transcribeSession } from "@/data/transcribe";
import { execute } from "@/db";
import { captureAnalytics } from "@/lib/analytics";
import { captureOperationalError } from "@/lib/error-reporting";
import { clearStaleMeetingRecordingActivities } from "@/live-activity/meeting-recording-activity";

const RECOVERY_CANDIDATES_SQL = `
SELECT session.id, json_extract(pending.value_json, '$.filename') AS pending_filename,
  json_extract(pending.value_json, '$.previousSizeBytes') AS previous_size_bytes
FROM sessions AS session
LEFT JOIN app_settings AS pending ON pending.id = 'mobile_capture_pending:' || session.id
WHERE session.deleted_at IS NULL
  AND (
    ? IS NULL
    OR session.owner_user_id = ?
    OR session.owner_user_id = (
      SELECT json_extract(value_json, '$.workspace_id')
      FROM app_settings
      WHERE id = 'cloudsync_workspace_binding'
    )
    OR session.owner_user_id IS NULL
    OR session.owner_user_id = '00000000-0000-0000-0000-000000000000'
  )
  AND (pending.id IS NOT NULL OR NOT EXISTS (
    SELECT 1
    FROM session_attachments AS attachment
    WHERE attachment.session_id = session.id
      AND attachment.source_type = 'session_audio'
      AND attachment.source_id = 'primary'
      AND attachment.deleted_at IS NULL
  ))
`;

void clearStaleMeetingRecordingActivities().catch((error) => {
  captureOperationalError(error, {
    operation: "recording_live_activity_cleanup",
  });
});

function repairInterruptedWav(file: File): number | null {
  if (!file.exists || file.size <= WAV_HEADER_BYTES) return null;

  const handle = file.open(FileMode.ReadWrite);
  try {
    handle.offset = 0;
    const format = recoverableWav(
      handle.readBytes(WAV_HEADER_BYTES),
      file.size,
    );
    if (!format) return null;

    handle.offset = 0;
    handle.writeBytes(
      wavHeader(format.dataBytes, format.sampleRate, format.channels),
    );
    return file.size;
  } finally {
    handle.close();
  }
}

export async function recoverInterruptedRecordings(
  accountUserId: string | null,
): Promise<number> {
  const sessions = await execute<{
    id: string;
    pending_filename: string | null;
    previous_size_bytes: number | null;
  }>(RECOVERY_CANDIDATES_SQL, [accountUserId, accountUserId]);
  const recoveredSessionIds: string[] = [];

  for (const session of sessions) {
    if (isMobileSessionCaptureActive(session.id)) continue;
    const filename = session.pending_filename ?? "audio.wav";
    if (
      filename !== "audio.wav" &&
      !/^audio-resume-[0-9a-f-]+\.wav$/.test(filename)
    )
      continue;
    const file = new File(Paths.document, "sessions", session.id, filename);
    try {
      if (
        session.pending_filename &&
        session.previous_size_bytes &&
        file.exists &&
        file.size <= session.previous_size_bytes
      ) {
        file.delete();
        await clearPendingCapture(session.id);
        continue;
      }
      const sizeBytes = repairInterruptedWav(file);
      if (sizeBytes === null) {
        if (
          session.pending_filename &&
          (!file.exists || file.size <= WAV_HEADER_BYTES)
        ) {
          if (file.exists) file.delete();
          await clearPendingCapture(session.id);
        }
        continue;
      }
      await catalogSessionAudio(session.id, {
        source: "recording",
        filename,
        contentType: "audio/wav",
      });
      recoveredSessionIds.push(session.id);
    } catch (error) {
      captureOperationalError(error, {
        operation: "recording_recovery",
        level: "warning",
        tags: { stage: "repair_or_catalog" },
      });
    }
  }

  if (recoveredSessionIds.length > 0) {
    captureAnalytics("recording_recovered", {
      recording_count: recoveredSessionIds.length,
    });
    for (const sessionId of recoveredSessionIds) {
      void transcribeSession(sessionId);
    }
  }
  return recoveredSessionIds.length;
}
