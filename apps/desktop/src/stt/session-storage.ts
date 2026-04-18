import { useCallback } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { TranscriptStorage } from "@hypr/store";

import { estimateUploadedAudioSessionCreatedAt } from "./audio-note-date";

import {
  useCurrentUserId,
  useMainStore,
  useSessionCellOptional,
  useSessionEvent,
  useSessionParticipantHumanIds,
  useTranscriptIdsForSession,
} from "~/session/hooks/storage";
import { id } from "~/shared/utils";
import type {
  LiveTranscriptPersistCallback,
  BatchPersistCallback,
} from "~/store/zustand/listener/transcript";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";
import { applyLiveTranscriptDelta } from "~/stt/utils";
import {
  parseTranscriptHints,
  parseTranscriptWords,
  updateTranscriptHints,
  updateTranscriptWords,
} from "~/stt/utils";

export function useLiveTranscriptPersistence(sessionId: string): {
  participantHumanIds: string[];
  hasCalendarEvent: boolean;
  persistDelta: (args: {
    currentTranscriptId: string | null;
    startedAt: number;
    createdAt: string;
    delta: Parameters<LiveTranscriptPersistCallback>[0];
  }) => string | null;
  rollbackTranscript: (transcriptId: string | null) => void;
} {
  const store = useMainStore();
  const userId = useCurrentUserId();
  const memoMd = useSessionCellOptional(sessionId, "raw_md");
  const participantHumanIds = useSessionParticipantHumanIds(sessionId);
  const hasCalendarEvent = useSessionEvent(sessionId) !== null;

  const persistDelta = useCallback(
    ({
      currentTranscriptId,
      startedAt,
      createdAt,
      delta,
    }: {
      currentTranscriptId: string | null;
      startedAt: number;
      createdAt: string;
      delta: Parameters<LiveTranscriptPersistCallback>[0];
    }) => {
      if (!store) return currentTranscriptId;
      if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
        return currentTranscriptId;
      }

      let transcriptId = currentTranscriptId;
      if (!transcriptId) {
        transcriptId = id();
        const transcriptRow = {
          session_id: sessionId,
          user_id: userId ?? "",
          created_at: createdAt,
          started_at: startedAt,
          words: "[]",
          speaker_hints: "[]",
          memo_md: typeof memoMd === "string" ? memoMd : "",
        } satisfies TranscriptStorage;

        store.setRow("transcripts", transcriptId, transcriptRow);
      }

      store.transaction(() => {
        applyLiveTranscriptDelta(store, transcriptId!, delta);
      });

      return transcriptId;
    },
    [memoMd, sessionId, store, userId],
  );

  const rollbackTranscript = useCallback(
    (transcriptId: string | null) => {
      if (!store || !transcriptId) return;
      store.delRow("transcripts", transcriptId);
    },
    [store],
  );

  return {
    participantHumanIds,
    hasCalendarEvent,
    persistDelta,
    rollbackTranscript,
  };
}

export function useBatchTranscriptPersistence(sessionId: string): {
  buildPersist: (
    provider: string,
    handlePersist?: BatchPersistCallback,
  ) => BatchPersistCallback;
} {
  const store = useMainStore();
  const userId = useCurrentUserId();
  const memoMd = useSessionCellOptional(sessionId, "raw_md");
  const transcriptIdsForSession = useTranscriptIdsForSession(sessionId);

  const buildPersist = useCallback(
    (provider: string, handlePersist?: BatchPersistCallback) => {
      if (handlePersist) {
        return handlePersist;
      }

      let transcriptId: string | null = null;
      const createdAt = new Date().toISOString();

      return (
        words: Parameters<BatchPersistCallback>[0],
        hints: Parameters<BatchPersistCallback>[1],
      ) => {
        if (!store || words.length === 0) {
          return;
        }

        if (!transcriptId) {
          transcriptId = id();
          const currentTranscriptId = transcriptId;

          const transcriptRow = {
            session_id: sessionId,
            user_id: userId ?? "",
            created_at: createdAt,
            started_at: Date.now(),
            words: "[]",
            speaker_hints: "[]",
            memo_md: typeof memoMd === "string" ? memoMd : "",
          } satisfies TranscriptStorage;

          store.transaction(() => {
            for (const existingTranscriptId of transcriptIdsForSession ?? []) {
              store.delRow("transcripts", existingTranscriptId);
            }

            store.setRow("transcripts", currentTranscriptId, transcriptRow);
          });
        }

        const currentTranscriptId = transcriptId;
        if (!currentTranscriptId) {
          return;
        }

        const existingWords = parseTranscriptWords(store, currentTranscriptId);
        const existingHints = parseTranscriptHints(store, currentTranscriptId);

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
              provider: hint.data.provider ?? provider,
              channel: hint.data.channel ?? word.channel,
              speaker_index: hint.data.speaker_index,
            }),
          });
        });

        updateTranscriptWords(store, currentTranscriptId, [
          ...existingWords,
          ...newWords,
        ]);
        updateTranscriptHints(store, currentTranscriptId, [
          ...existingHints,
          ...newHints,
        ]);
      };
    },
    [memoMd, sessionId, store, transcriptIdsForSession, userId],
  );

  return { buildPersist };
}

export function useUploadTranscriptImport(sessionId: string): {
  applyEstimatedAudioNoteDate: (filePath: string) => Promise<void>;
  importSubtitleTokens: (
    tokens: Array<{ text: string; start_time: number; end_time: number }>,
  ) => void;
} {
  const store = useMainStore();
  const userId = useCurrentUserId();
  const rawMd = useSessionCellOptional(sessionId, "raw_md");
  const event = useSessionEvent(sessionId);

  const applyEstimatedAudioNoteDate = useCallback(
    async (filePath: string) => {
      try {
        if (!store || event) {
          return;
        }

        const result = await fsSyncCommands.audioSourceMetadata(filePath);
        if (result.status === "error") {
          return;
        }

        const estimatedCreatedAt = estimateUploadedAudioSessionCreatedAt(
          result.data,
        );
        if (!estimatedCreatedAt) {
          return;
        }

        store.setCell("sessions", sessionId, "created_at", estimatedCreatedAt);
      } catch (error) {
        console.error("[upload] audio metadata inspection failed:", error);
      }
    },
    [event, sessionId, store],
  );

  const importSubtitleTokens = useCallback(
    (tokens: Array<{ text: string; start_time: number; end_time: number }>) => {
      if (!store || tokens.length === 0) {
        return;
      }

      const transcriptId = crypto.randomUUID();
      const createdAt = new Date().toISOString();

      const words = tokens.map((token) => ({
        id: crypto.randomUUID(),
        transcript_id: transcriptId,
        text: token.text,
        start_ms: token.start_time,
        end_ms: token.end_time,
        channel: 2,
        user_id: userId ?? "",
        created_at: new Date().toISOString(),
      }));

      const transcriptRow = {
        session_id: sessionId,
        user_id: userId ?? "",
        created_at: createdAt,
        started_at: Date.now(),
        words: JSON.stringify(words),
        speaker_hints: "[]",
        memo_md: typeof rawMd === "string" ? rawMd : "",
      } satisfies TranscriptStorage;

      store.setRow("transcripts", transcriptId, transcriptRow);
    },
    [rawMd, sessionId, store, userId],
  );

  return { applyEstimatedAudioNoteDate, importSubtitleTokens };
}
