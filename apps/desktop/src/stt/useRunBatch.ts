import { useCallback } from "react";

import type { BatchParams } from "@hypr/plugin-transcription";
import type { TranscriptStorage } from "@hypr/store";

import { useListener } from "./contexts";
import { useKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import type { BatchPersistCallback } from "~/store/zustand/listener/transcript";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import {
  parseTranscriptHints,
  parseTranscriptWords,
  updateTranscriptHints,
  updateTranscriptWords,
} from "~/stt/utils";

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

const BATCH_PROVIDER_MAP: Record<string, BatchParams["provider"]> = {
  deepgram: "deepgram",
  soniox: "soniox",
  assemblyai: "assemblyai",
  openai: "openai",
  gladia: "gladia",
  elevenlabs: "elevenlabs",
  mistral: "mistral",
  fireworks: "fireworks",
};

function getBatchProvider(
  provider: string,
  model: string,
): BatchParams["provider"] | null {
  if (provider === "hyprnote") {
    if (model.startsWith("am-")) return "am";
    if (model.startsWith("cactus-")) return "cactus";
    return "hyprnote";
  }
  return BATCH_PROVIDER_MAP[provider] ?? null;
}

export const useRunBatch = (sessionId: string) => {
  const store = main.UI.useStore(main.STORE_ID);
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const runBatch = useListener((state) => state.runBatch);
  const { conn } = useSTTConnection();
  const keywords = useKeywords(sessionId);
  const languages = useConfigValue("spoken_languages");

  return useCallback(
    async (filePath: string, options?: RunOptions) => {
      if (!store || !conn || !runBatch) {
        throw new Error(
          "STT connection is not available. Please configure your speech-to-text provider.",
        );
      }

      const provider = getBatchProvider(conn.provider, conn.model);

      if (!provider) {
        throw new Error(
          `Batch transcription is not supported for provider: ${conn.provider}`,
        );
      }

      const createdAt = new Date().toISOString();
      const memoMd = store.getCell("sessions", sessionId, "raw_md");
      let transcriptId: string | null = null;

      const handlePersist: BatchPersistCallback | undefined =
        options?.handlePersist;

      const persist =
        handlePersist ??
        ((words, hints) => {
          if (words.length === 0) {
            return;
          }

          if (!transcriptId) {
            transcriptId = id();

            const transcriptRow = {
              session_id: sessionId,
              user_id: user_id ?? "",
              created_at: createdAt,
              started_at: Date.now(),
              words: "[]",
              speaker_hints: "[]",
              memo_md: typeof memoMd === "string" ? memoMd : "",
            } satisfies TranscriptStorage;

            store.setRow("transcripts", transcriptId, transcriptRow);
          }

          const existingWords = parseTranscriptWords(store, transcriptId);
          const existingHints = parseTranscriptHints(store, transcriptId);

          const newWords: WordWithId[] = [];
          const newWordIds: string[] = [];

          words.forEach((word) => {
            const wordId = id();

            newWords.push({
              id: wordId,
              text: word.text,
              start_ms: word.start_ms,
              end_ms: word.end_ms,
              channel: word.channel,
            });

            newWordIds.push(wordId);
          });

          const newHints: SpeakerHintWithId[] = [];

          hints.forEach((hint) => {
            if (hint.data.type !== "provider_speaker_index") {
              return;
            }

            const wordId = newWordIds[hint.wordIndex];
            const word = words[hint.wordIndex];

            if (!wordId || !word) {
              return;
            }

            newHints.push({
              id: id(),
              word_id: wordId,
              type: "provider_speaker_index",
              value: JSON.stringify({
                provider: hint.data.provider ?? conn.provider,
                channel: hint.data.channel ?? word.channel,
                speaker_index: hint.data.speaker_index,
              }),
            });
          });

          updateTranscriptWords(store, transcriptId, [
            ...existingWords,
            ...newWords,
          ]);
          updateTranscriptHints(store, transcriptId, [
            ...existingHints,
            ...newHints,
          ]);
        });

      const params: BatchParams = {
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

      await runBatch(params, { handlePersist: persist });
    },
    [conn, keywords, languages, runBatch, sessionId, store, user_id],
  );
};
