import { SessionWavWriter } from "@/audio/session-wav-writer";
import { isSessionTranscribing } from "@/data/transcribe";
import { execute } from "@/db";

export class RecordingResumeError extends Error {}

export async function prepareSessionRecording(sessionId: string) {
  const [session] = await execute<{
    pending_filename: string | null;
    has_transcript: number;
  }>(
    `SELECT (SELECT json_extract(value_json, '$.filename') FROM app_settings WHERE id = 'mobile_capture_pending:' || sessions.id) AS pending_filename,
    EXISTS (SELECT 1 FROM transcripts WHERE session_id = sessions.id AND deleted_at IS NULL) AS has_transcript
    FROM sessions WHERE id = ? AND deleted_at IS NULL`,
    [sessionId],
  );
  if (!session)
    throw new RecordingResumeError("This note is no longer available.");
  if (session.pending_filename)
    throw new RecordingResumeError(
      "Reopen the app to recover the interrupted recording before resuming.",
    );
  if (isSessionTranscribing(sessionId))
    throw new RecordingResumeError(
      "Wait for transcription to finish before resuming.",
    );
  const [audio] = await execute<{
    relative_path: string | null;
    transcript_status: string | null;
    capture_started_at_ms: number | null;
  }>(
    `SELECT local.relative_path,
    json_extract(audio.metadata_json, '$.transcript_status') AS transcript_status,
    COALESCE(json_extract(audio.metadata_json, '$.capture_started_at_ms'),
      (SELECT MIN(started_at_ms) FROM transcripts WHERE session_id = audio.session_id AND deleted_at IS NULL)) AS capture_started_at_ms
    FROM session_attachments AS audio
    LEFT JOIN attachment_local_state AS local ON local.attachment_id = audio.id AND local.availability = 'present'
    WHERE audio.session_id = ? AND audio.source_type = 'session_audio' AND audio.source_id = 'primary' AND audio.deleted_at IS NULL`,
    [sessionId],
  );
  if (
    audio &&
    (!audio.relative_path || audio.transcript_status !== "complete")
  ) {
    throw new RecordingResumeError(
      "Download the recording and finish its transcription before resuming.",
    );
  }
  if (session.has_transcript && !audio)
    throw new RecordingResumeError(
      "The original recording is needed to resume this note.",
    );
  if (audio && !Number.isFinite(audio.capture_started_at_ms))
    throw new RecordingResumeError(
      "This recording has no capture time and cannot be resumed.",
    );
  try {
    const writer = new SessionWavWriter(
      sessionId,
      audio?.relative_path ?? undefined,
    );
    return { writer, startedAtMs: audio?.capture_started_at_ms ?? Date.now() };
  } catch (error) {
    if (audio)
      throw new RecordingResumeError(
        error instanceof Error
          ? error.message
          : "This recording cannot be resumed.",
      );
    throw error;
  }
}
