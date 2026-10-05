import { commands as listenerCommands } from "@anlg/plugin-transcription";
import type { CaptureLifecycleMarker } from "@anlg/plugin-transcription";

import { enqueueDatabaseWrite } from "~/db/write-queue";

export type {
  CaptureLifecycleMarker,
  InheritedCapture,
} from "@anlg/plugin-transcription";

export const CAPTURE_LIFECYCLE_SETTING_PREFIX = "capture_lifecycle_pending:";
export const CAPTURE_AUDIO_SAVED_SETTING_PREFIX = "capture_audio_saved:";

export function saveCaptureLifecycleMarker(
  marker: CaptureLifecycleMarker,
  replaceTranscriptId?: string,
): Promise<void> {
  return enqueueDatabaseWrite(`session:${marker.sessionId}`, async () => {
    const result = await listenerCommands.saveCaptureLifecycleMarker(
      marker,
      replaceTranscriptId ?? null,
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function clearCaptureLifecycleMarker(
  sessionId: string,
  transcriptId: string,
): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await listenerCommands.clearCaptureLifecycleMarker(
      sessionId,
      transcriptId,
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

// A stopped capture whose saved audio waits for the user to create the note
// or resume listening.
export function markCaptureAudioSaved(sessionId: string): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await listenerCommands.markCaptureAudioSaved(sessionId);
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function clearCaptureAudioSaved(sessionId: string): Promise<void> {
  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const result = await listenerCommands.clearCaptureAudioSaved(sessionId);
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export async function loadCaptureLifecycleMarker(
  sessionId: string,
): Promise<CaptureLifecycleMarker | null> {
  const result = await listenerCommands.getCaptureLifecycleMarker(sessionId);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function hasAudioAwaitingUser(marker: CaptureLifecycleMarker) {
  return (
    !marker.summaryMode &&
    (marker.chunkedAudio === true ||
      (marker.inheritedCaptures ?? []).length > 0)
  );
}

export function hasPendingZeroRetentionAudio(marker: CaptureLifecycleMarker) {
  return (
    (marker.chunkedAudio === true && marker.retainAudio === false) ||
    (marker.inheritedCaptures ?? []).some(
      (capture) => capture.retainAudio === false,
    )
  );
}
