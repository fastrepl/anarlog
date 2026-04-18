import { useCallback } from "react";

import type { TranscriptionParams } from "@hypr/plugin-transcription";

import { useListener } from "./contexts";
import { useBatchTranscriptPersistence } from "./session-storage";
import { useKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { useConfigValue } from "~/shared/config";
import type { BatchPersistCallback } from "~/store/zustand/listener/transcript";

type RunOptions = {
  handlePersist?: BatchPersistCallback;
  model?: string;
  baseUrl?: string;
  apiKey?: string;
  keywords?: string[];
  languages?: string[];
  numSpeakers?: number;
  minSpeakers?: number;
  maxSpeakers?: number;
};

const DIRECT_BATCH_PROVIDERS: Set<TranscriptionParams["provider"]> = new Set([
  "deepgram",
  "soniox",
  "assemblyai",
  "openai",
  "gladia",
  "elevenlabs",
  "mistral",
  "fireworks",
  "pyannote",
  "aquavoice",
]);

export const STOPPED_TRANSCRIPTION_ERROR_MESSAGE = "Transcription stopped.";

export function getBatchProvider(
  provider: string,
  model: string,
): TranscriptionParams["provider"] | null {
  if (provider === "hyprnote") {
    if (model.startsWith("am-")) return "am";
    if (model.startsWith("cactus-")) return "cactus";
    return "hyprnote";
  }
  if (DIRECT_BATCH_PROVIDERS.has(provider as TranscriptionParams["provider"])) {
    return provider as TranscriptionParams["provider"];
  }
  return null;
}

export function canRunBatchTranscription(
  conn: { provider: string; model: string } | null,
  modelOverride?: string,
) {
  if (!conn) {
    return false;
  }

  return getBatchProvider(conn.provider, modelOverride ?? conn.model) != null;
}

export function isStoppedTranscriptionError(error: unknown) {
  return (
    (error instanceof Error ? error.message : String(error)) ===
    STOPPED_TRANSCRIPTION_ERROR_MESSAGE
  );
}

export const useRunBatch = (sessionId: string) => {
  const { buildPersist } = useBatchTranscriptPersistence(sessionId);

  const startTranscription = useListener((state) => state.startTranscription);
  const { conn } = useSTTConnection();
  const keywords = useKeywords(sessionId);
  const languages = useConfigValue("spoken_languages");

  return useCallback(
    async (filePath: string, options?: RunOptions) => {
      if (!conn || !startTranscription) {
        throw new Error(
          "STT connection is not available. Please configure your speech-to-text provider.",
        );
      }

      const provider = getBatchProvider(
        conn.provider,
        options?.model ?? conn.model,
      );

      if (!provider) {
        throw new Error(
          `Batch transcription is not supported for provider: ${conn.provider}`,
        );
      }

      const handlePersist: BatchPersistCallback | undefined =
        options?.handlePersist;

      const persist = buildPersist(conn.provider, handlePersist);

      const params: TranscriptionParams = {
        session_id: sessionId,
        provider,
        file_path: filePath,
        model: options?.model ?? conn.model,
        base_url: options?.baseUrl ?? conn.baseUrl,
        api_key: options?.apiKey ?? conn.apiKey,
        keywords: options?.keywords ?? keywords ?? [],
        languages: options?.languages ?? languages ?? [],
        num_speakers: options?.numSpeakers,
        min_speakers: options?.minSpeakers,
        max_speakers: options?.maxSpeakers,
      };

      await startTranscription(params, { handlePersist: persist });
    },
    [conn, buildPersist, keywords, languages, startTranscription, sessionId],
  );
};
