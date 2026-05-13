import { commands as listenerCommands } from "@hypr/plugin-transcription";

export function isRealtimeLocalModel(model?: string | null) {
  return model === "soniqo-parakeet-streaming";
}

export function getOnDeviceTranscriptionMode(model: string | null | undefined) {
  if (!isRealtimeLocalModel(model)) {
    return "batch" as const;
  }

  return "live" as const;
}

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
