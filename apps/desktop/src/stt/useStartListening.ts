import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { RecordingMode, TranscriptionMode } from "@hypr/plugin-listener";
import type { TranscriptStorage } from "@hypr/store";

import { useListener } from "./contexts";
import { useKeywords } from "./useKeywords";
import { useSTTConnection } from "./useSTTConnection";

import { getEnhancerService } from "~/services/enhancer";
import { getSessionEventById } from "~/session/utils";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import type {
  HandlePersistCallback,
  OnStoppedCallback,
} from "~/store/zustand/listener/transcript";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import {
  parseTranscriptHints,
  parseTranscriptWords,
  updateTranscriptHints,
  updateTranscriptWords,
} from "~/stt/utils";

const MIN_DURATION_SECONDS = 10;
const MIN_WORD_COUNT = 5;

export function useStartListening(
  sessionId: string,
  options?: {
    transcriptionMode?: TranscriptionMode;
    recordingMode?: RecordingMode;
  },
) {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);

  const record_enabled = useConfigValue("save_recordings");
  const languages = useConfigValue("spoken_languages");

  const start = useListener((state) => state.start);
  const { conn } = useSTTConnection();

  const keywords = useKeywords(sessionId);
  const transcriptionMode = options?.transcriptionMode ?? "live";
  const recordingMode =
    options?.recordingMode ?? (record_enabled ? "disk" : "memory");

  const startListening = useCallback(async () => {
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

    const onStopped: OnStoppedCallback = (_sessionId, durationSeconds) => {
      const words = parseTranscriptWords(store, transcriptId);

      if (
        durationSeconds < MIN_DURATION_SECONDS &&
        words.length < MIN_WORD_COUNT
      ) {
        store.transaction(() => {
          store.delRow("transcripts", transcriptId);

          if (indexes) {
            const enhancedNoteIds = indexes.getSliceRowIds(
              main.INDEXES.enhancedNotesBySession,
              sessionId,
            );
            for (const noteId of enhancedNoteIds) {
              store.delRow("enhanced_notes", noteId);
            }
          }
        });

        void fsSyncCommands.audioDelete(sessionId);

        const tabsState = useTabs.getState();
        const sessionTab = tabsState.tabs.find(
          (t): t is Extract<Tab, { type: "sessions" }> =>
            t.type === "sessions" && t.id === sessionId,
        );
        if (sessionTab) {
          tabsState.updateSessionTabState(sessionTab, {
            ...sessionTab.state,
            view: null,
          });
        }
        return;
      }

      getEnhancerService()?.queueAutoEnhance(sessionId);
    };

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

    const started = await start(
      {
        session_id: sessionId,
        languages,
        onboarding: false,
        transcription_mode: transcriptionMode,
        recording_mode: recordingMode,
        model: conn.model,
        base_url: conn.baseUrl,
        api_key: conn.apiKey,
        keywords,
      },
      {
        handlePersist,
        onStopped,
      },
    );

    if (!started) {
      store.delRow("transcripts", transcriptId);
      return;
    }

    void analyticsCommands.event({
      event: "session_started",
      has_calendar_event: !!getSessionEventById(store, sessionId),
      stt_provider: conn.provider,
      stt_model: conn.model,
    });
  }, [
    conn,
    store,
    indexes,
    sessionId,
    start,
    keywords,
    user_id,
    languages,
    recordingMode,
    transcriptionMode,
  ]);

  return startListening;
}
