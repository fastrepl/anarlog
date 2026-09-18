import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { liveQueryClient } from "~/db";
import { enqueueSessionAudioOperation } from "~/session/audio-operations";
import { flushSessionEditorChanges } from "~/session/editor-registry";
import { isSessionEmpty } from "~/session/queries";

export async function discardEmptyAutomaticCapture({
  sessionId,
  automatic,
  preserveExistingAudio,
  preserveExistingTranscript,
  initialTitle,
  transcriptTouched,
  transcriptionComplete,
}: {
  sessionId: string;
  automatic: boolean;
  preserveExistingAudio: boolean;
  preserveExistingTranscript: boolean;
  initialTitle: string | undefined;
  transcriptTouched: boolean;
  transcriptionComplete: boolean;
}): Promise<boolean> {
  if (
    !automatic ||
    preserveExistingAudio ||
    preserveExistingTranscript ||
    initialTitle === undefined ||
    transcriptTouched ||
    !transcriptionComplete
  ) {
    return false;
  }
  try {
    await flushSessionEditorChanges(sessionId);
    return await enqueueSessionAudioOperation(sessionId, async () => {
      const speech = await fsSyncCommands.audioHasSpeech(sessionId);
      if (speech.status !== "ok" || speech.data) return false;
      await flushSessionEditorChanges(sessionId);
      const [session] = await liveQueryClient.execute<{
        title: string;
        has_attachments: boolean | number;
      }>(
        `SELECT title, EXISTS (
          SELECT 1 FROM session_attachments
          WHERE session_id = sessions.id AND deleted_at IS NULL
        ) AS has_attachments
        FROM sessions WHERE id = ? AND deleted_at IS NULL`,
        [sessionId],
      );
      if (
        !session ||
        session.title !== initialTitle ||
        (session.has_attachments !== 0 && session.has_attachments !== false) ||
        !(await isSessionEmpty(sessionId))
      ) {
        return false;
      }
      // Only remove this device's un-catalogued audio. The calendar note and
      // anything another device has contributed stay in the shared database.
      const result = await fsSyncCommands.audioDelete(sessionId);
      return result.status === "ok" && result.data;
    });
  } catch (error) {
    console.warn(
      "[listener] keeping automatic capture after an inconclusive activity check",
      error,
    );
    return false;
  }
}
