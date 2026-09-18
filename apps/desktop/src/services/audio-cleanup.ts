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
    sonnerToast.error(
      payload.session_id
        ? "Audio could not be deleted"
        : "Audio cleanup could not finish",
      {
        id: payload.session_id
          ? `audio-deletion-${payload.session_id}`
          : "audio-cleanup",
        duration: Infinity,
        description: payload.session_id
          ? "BlackMushi could not remove temporary audio and will retry cleanup automatically."
          : "BlackMushi could not finish audio cleanup or recovery and will retry automatically.",
      },
    );
    if (payload.session_id) {
      await saveIncompleteCapture(
        payload.session_id,
        "audio-cleanup",
        false,
        true,
      );
    }
  } else if (payload.error.startsWith("audio_recovery_failed:")) {
    sonnerToast.error("Audio could not be recovered", {
      id: `audio-recovery-${payload.session_id}`,
      duration: Infinity,
      description:
        "BlackMushi could not restore interrupted audio and will retry recovery automatically.",
    });
    if (payload.session_id) {
      await saveIncompleteCapture(payload.session_id, "audio-recovery", false);
    }
  } else if (payload.error === "audio_recovery_completed") {
    // Restoring audio does not prove that the missing transcript was repaired.
    sonnerToast.dismiss(`audio-recovery-${payload.session_id}`);
  } else if (payload.error === "audio_deletion_completed") {
    if (payload.session_id) {
      await clearCaptureAudioDeletionFailure(payload.session_id);
      sonnerToast.dismiss(`audio-deletion-${payload.session_id}`);
    } else {
      sonnerToast.dismiss("audio-cleanup");
    }
  } else {
    return;
  }
  const result = await commands.acknowledgeCaptureAudioCleanupStatus(
    payload.session_id,
    payload.error,
  );
  if (result.status === "error") throw new Error(result.error);
}

export async function listenForCaptureCleanup() {
  let changedDuringRead: Set<string> | undefined = new Set();
  let pending = Promise.resolve();
  const consume = (payload: CaptureStatusEvent) => {
    pending = pending
      .then(() => handleCaptureCleanupStatus(payload))
      .catch((error) => {
        console.error(
          "[audio-cleanup] failed to persist cleanup status",
          error,
        );
      });
    return pending;
  };
  const unlisten = await events.captureStatusEvent
    .listen(({ payload }) => {
      if (
        payload.type === "audio_error" &&
        (payload.error.startsWith("audio_deletion_failed:") ||
          payload.error.startsWith("audio_recovery_failed:") ||
          payload.error === "audio_deletion_completed" ||
          payload.error === "audio_recovery_completed")
      ) {
        changedDuringRead?.add(payload.session_id);
      }
      void consume(payload);
    })
    .catch((error) => {
      console.error(
        "[audio-cleanup] failed to subscribe to cleanup status",
        error,
      );
      return () => {};
    });
  try {
    const result = await commands.getCaptureAudioCleanupStatus();
    if (result.status === "error") throw new Error(result.error);
    for (const [sessionId, error] of Object.entries(result.data)) {
      if (!error || changedDuringRead.has(sessionId)) continue;
      await consume({
        type: "audio_error",
        session_id: sessionId,
        is_fatal: false,
        device: null,
        error,
      });
    }
  } catch (error) {
    console.error("[audio-cleanup] failed to read cleanup status", error);
  } finally {
    changedDuringRead = undefined;
  }
  return unlisten;
}
