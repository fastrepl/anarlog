import { commands as listenerCommands } from "@hypr/plugin-transcription";

export async function isLiveTranscriptionSupported(
  provider?: string | null,
  model?: string | null,
) {
  if (!provider || !model) {
    return false;
  }

  const result = await listenerCommands.isSupportedLanguagesLive(
    provider,
    model,
    [],
  );

  return result.status === "ok" ? result.data : true;
}
