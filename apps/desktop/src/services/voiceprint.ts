import { commands as transcriptionCommands } from "@anlg/plugin-transcription";

const CLEANUP_INTERVAL_MS = 6 * 60 * 60 * 1000;

let lastCleanupMs = 0;

// Piggybacks on the frequent audio-retention tick but only sweeps a few
// times a day: expired candidates are a privacy cleanup, not a hot path.
export async function cleanupExpiredVoiceprintCandidates(
  nowMs = Date.now(),
): Promise<void> {
  if (nowMs - lastCleanupMs < CLEANUP_INTERVAL_MS) {
    return;
  }
  lastCleanupMs = nowMs;

  try {
    const result =
      await transcriptionCommands.cleanupExpiredVoiceprintCandidates();
    if (result.status === "error") {
      console.error("[voiceprint] candidate cleanup failed", result.error);
    }
  } catch (error) {
    console.error("[voiceprint] candidate cleanup failed", error);
  }
}

// Runs after a transcript is persisted and before audio retention may delete
// the recording — the embeddings must outlive the audio. Failures are logged
// and swallowed: losing candidates for one session is acceptable, blocking
// transcription completion is not.
export async function maybeExtractVoiceprintCandidates(input: {
  enabled: boolean;
  sessionId: string;
  transcriptId: string;
  audioPath: string | null | undefined;
}): Promise<void> {
  if (!input.enabled || !input.audioPath) {
    return;
  }

  try {
    const result = await transcriptionCommands.extractVoiceprintCandidates(
      input.sessionId,
      input.transcriptId,
      input.audioPath,
    );
    if (result.status === "error") {
      console.error("[voiceprint] candidate extraction failed", result.error);
    }
  } catch (error) {
    console.error("[voiceprint] candidate extraction failed", error);
  }
}
