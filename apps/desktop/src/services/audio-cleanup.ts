import {
  commands,
  events,
  type CaptureStatusEvent,
} from "@anlg/plugin-transcription";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  clearCaptureAudioDeletionFailure,
  saveIncompleteCapture,
} from "~/stt/capture-result";

export async function handleCaptureCleanupStatus(payload: CaptureStatusEvent) {
  if (payload.type !== "audio_error") return;
  if (payload.error.startsWith("audio_deletion_failed:")) {
    sonnerToast.error("Audio could not be deleted", {
      id: "audio-cleanup",
      duration: Infinity,
      description:
        "Anarlog could not remove temporary audio and will retry cleanup automatically.",
    });
    if (payload.session_id) {
      await saveIncompleteCapture(
        payload.session_id,
        "audio-cleanup",
        false,
        true,
      );
    }
  } else if (payload.error === "audio_deletion_completed") {
    if (payload.session_id) {
      await clearCaptureAudioDeletionFailure(payload.session_id);
      sonnerToast.dismiss(`audio-deletion-${payload.session_id}`);
    } else {
      sonnerToast.dismiss("audio-cleanup");
    }
  }
}

export async function listenForCaptureCleanup() {
  let changedDuringRead: Set<string> | undefined = new Set();
  const unlisten = await events.captureStatusEvent.listen(({ payload }) => {
    if (
      payload.type === "audio_error" &&
      (payload.error.startsWith("audio_deletion_failed:") ||
        payload.error === "audio_deletion_completed")
    ) {
      changedDuringRead?.add(payload.session_id);
    }
    void handleCaptureCleanupStatus(payload).catch((error) => {
      console.error("[audio-cleanup] failed to persist cleanup status", error);
    });
  });
  try {
    const result = await commands.getCaptureAudioCleanupStatus();
    if (result.status === "error") throw new Error(result.error);
    for (const [sessionId, failed] of Object.entries(result.data)) {
      if (changedDuringRead.has(sessionId)) continue;
      await handleCaptureCleanupStatus({
        type: "audio_error",
        session_id: sessionId,
        is_fatal: false,
        device: null,
        error: failed
          ? "audio_deletion_failed: startup cleanup failed"
          : "audio_deletion_completed",
      });
    }
  } catch (error) {
    console.error("[audio-cleanup] failed to read cleanup status", error);
  } finally {
    changedDuringRead = undefined;
  }
  return unlisten;
}
