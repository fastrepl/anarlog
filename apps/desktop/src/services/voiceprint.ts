import { commands as transcriptionCommands } from "@anlg/plugin-transcription";

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
