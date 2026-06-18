import { useCallback } from "react";

import type { TranscriptionParams } from "@hypr/plugin-transcription";
import type { TranscriptStorage } from "@hypr/store";

import { useListener } from "./contexts";
import { useKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { deleteProcessedAudioForRetention } from "~/services/audio-retention";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";
import type { BatchPersistCallback } from "~/store/zustand/listener/transcript";
import { getTranscriptionLanguages } from "~/stt/capabilities";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import {
  createTranscriptAccumulator,
  type TranscriptAccumulator,
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

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

const DIRECT_BATCH_PROVIDERS: Set<TranscriptionParams["provider"]> = new Set([
  "deepgram",
  "cartesia",
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
  if (provider === "cloudflare_workers_ai") {
    return "deepgram";
  }

  if (provider === "hyprnote") {
    if (model.startsWith("soniqo-")) return "soniqo";
    if (model.startsWith("am-")) return "am";
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

export function getSessionSpeakerCount(
  store: Store,
  sessionId: string,
  selfHumanId?: string | null,
): number | undefined {
  const humanIds = new Set<string>();

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const sid = store.getCell(
      "mapping_session_participant",
      mappingId,
      "session_id",
    );
    if (sid !== sessionId) return;

    const humanId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "human_id",
    );
    if (typeof humanId === "string" && humanId) {
      humanIds.add(humanId);
    }
  });

  if (typeof selfHumanId === "string" && selfHumanId) {
    humanIds.add(selfHumanId);
  }

  return humanIds.size > 1 ? humanIds.size : undefined;
}

async function saveCompletedBatchTranscript(): Promise<void> {
  try {
    const { save } = await import("~/store/tinybase/store/save");
    await save();
  } catch (error) {
    console.error("[runBatch] failed to save completed transcript", error);
  }
}

export const useRunBatch = (sessionId: string) => {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);

  const startTranscription = useListener((state) => state.startTranscription);
  const { conn } = useSTTConnection();
  const keywords = useKeywords(sessionId);
  const aiLanguage = useConfigValue("ai_language");
  const spokenLanguages = useConfigValue("spoken_languages");

  return useCallback(
    async (filePath: string, options?: RunOptions) => {
      if (!store || !conn || !startTranscription) {
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

      const createdAt = new Date().toISOString();
      const memoMd = store.getCell("sessions", sessionId, "raw_md");
      let transcriptId: string | null = null;
      const inferredNumSpeakers =
        options?.numSpeakers === undefined &&
        options?.minSpeakers === undefined &&
        options?.maxSpeakers === undefined
          ? getSessionSpeakerCount(store, sessionId, user_id)
          : undefined;

      const handlePersist: BatchPersistCallback | undefined =
        options?.handlePersist;
      let wroteDefaultTranscript = false;
      const transcriptAccumulatorRef: {
        current: TranscriptAccumulator | null;
      } = { current: null };

      const persist =
        handlePersist ??
        ((words, hints, persistOptions) => {
          if (words.length === 0) {
            return;
          }

          if (!transcriptId) {
            transcriptId = id();
            const currentTranscriptId = transcriptId;

            const transcriptRow = {
              session_id: sessionId,
              user_id: user_id ?? "",
              created_at: createdAt,
              started_at: Date.now(),
              words: "[]",
              speaker_hints: "[]",
              memo_md: typeof memoMd === "string" ? memoMd : "",
            } satisfies TranscriptStorage;

            store.transaction(() => {
              const transcriptIds =
                indexes?.getSliceRowIds(
                  main.INDEXES.transcriptBySession,
                  sessionId,
                ) ?? [];

              for (const existingTranscriptId of transcriptIds) {
                store.delRow("transcripts", existingTranscriptId);
              }

              store.setRow("transcripts", currentTranscriptId, transcriptRow);
            });

            transcriptAccumulatorRef.current = createTranscriptAccumulator(
              store,
              currentTranscriptId,
              { words: [], hints: [] },
            );
          }

          const currentTranscriptId = transcriptId;
          if (!currentTranscriptId) {
            return;
          }

          transcriptAccumulatorRef.current ??= createTranscriptAccumulator(
            store,
            currentTranscriptId,
          );

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
              metadata: word.metadata
                ? JSON.stringify(word.metadata)
                : undefined,
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

          store.transaction(() => {
            transcriptAccumulatorRef.current?.appendWordsAndHints(
              newWords,
              newHints,
              persistOptions,
            );
          });

          wroteDefaultTranscript = true;
        });

      const params: TranscriptionParams = {
        session_id: sessionId,
        provider,
        file_path: filePath,
        model: options?.model ?? conn.model,
        base_url: options?.baseUrl ?? conn.baseUrl,
        api_key: options?.apiKey ?? conn.apiKey,
        keywords: options?.keywords ?? keywords ?? [],
        languages:
          options?.languages ??
          getTranscriptionLanguages(aiLanguage, spokenLanguages),
        num_speakers: options?.numSpeakers ?? inferredNumSpeakers,
        min_speakers: options?.minSpeakers,
        max_speakers: options?.maxSpeakers,
      };

      try {
        await startTranscription(params, { handlePersist: persist });
      } finally {
        if (!handlePersist && wroteDefaultTranscript) {
          await saveCompletedBatchTranscript();
        }

        transcriptAccumulatorRef.current?.dispose();
        transcriptAccumulatorRef.current = null;
      }

      if (settingsStore) {
        await deleteProcessedAudioForRetention(
          store as main.Store,
          settingsStore as settings.Store,
          sessionId,
        );
      }
    },
    [
      conn,
      aiLanguage,
      indexes,
      keywords,
      spokenLanguages,
      startTranscription,
      sessionId,
      settingsStore,
      store,
      user_id,
    ],
  );
};
