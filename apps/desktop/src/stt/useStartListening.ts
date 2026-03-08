import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type { TranscriptStorage } from "@hypr/store";

import { useListener } from "./contexts";
import { useKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { getSessionEventById } from "~/session/utils";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import type { HandlePersistCallback } from "~/store/zustand/listener/transcript";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import {
  parseTranscriptHints,
  parseTranscriptWords,
  updateTranscriptHints,
  updateTranscriptWords,
} from "~/stt/utils";

export function useStartListening(sessionId: string) {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);

  const record_enabled = useConfigValue("save_recordings");
  const languages = useConfigValue("spoken_languages");

  const start = useListener((state) => state.start);
  const { conn } = useSTTConnection();

  const keywords = useKeywords(sessionId);

  const startListening = useCallback(() => {
    if (!conn || !store) {
      console.error("no_stt_connection");
      return;
    }

    const transcriptId = id();
    const startedAt = Date.now();
    const memoMd = store.getCell("sessions", sessionId, "raw_md");
    const transcriptRow = {
      session_id: sessionId,
      user_id: user_id ?? "",
      created_at: new Date().toISOString(),
      started_at: startedAt,
      words: "[]",
      speaker_hints: "[]",
      memo_md: typeof memoMd === "string" ? memoMd : "",
    } satisfies TranscriptStorage;

    store.setRow("transcripts", transcriptId, transcriptRow);

    void analyticsCommands.event({
      event: "session_started",
      has_calendar_event: !!getSessionEventById(store, sessionId),
      stt_provider: conn.provider,
      stt_model: conn.model,
    });

    const handlePersist: HandlePersistCallback = (words, hints) => {
      if (words.length === 0) {
        return;
      }

      store.transaction(() => {
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

        if (conn.provider === "deepgram") {
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
        }

        updateTranscriptWords(store, transcriptId, [
          ...existingWords,
          ...newWords,
        ]);
        updateTranscriptHints(store, transcriptId, [
          ...existingHints,
          ...newHints,
        ]);
      });
    };

    start(
      {
        session_id: sessionId,
        languages,
        onboarding: false,
        record_enabled,
        model: conn.model,
        base_url: conn.baseUrl,
        api_key: conn.apiKey,
        keywords,
      },
      {
        handlePersist,
      },
    );
  }, [
    conn,
    store,
    sessionId,
    start,
    keywords,
    user_id,
    record_enabled,
    languages,
  ]);

  return startListening;
}
