import { useCallback, useRef } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { TranscriptStorage } from "@hypr/store";

import { useListener } from "./contexts";
import { useKeywords } from "./useKeywords";
import {
  canRunBatchTranscription,
  isStoppedTranscriptionError,
  useRunBatch,
} from "./useRunBatch";
import { useSTTConnection } from "./useSTTConnection";

import { getEnhancerService } from "~/services/enhancer";
import { getSessionEventById } from "~/session/utils";
import { useConfigValue } from "~/shared/config";
import { id } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";
import type {
  LiveTranscriptPersistCallback,
  OnStoppedCallback,
} from "~/store/zustand/listener/transcript";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { applyLiveTranscriptDelta, parseTranscriptWords } from "~/stt/utils";

const MIN_DURATION_SECONDS = 10;
const MIN_WORD_COUNT = 5;

export function getPostCaptureAction(
  details: {
    audioPath: string | null;
    liveTranscriptionActive: boolean;
  },
  canRunBatch: boolean,
) {
  if (details.liveTranscriptionActive) {
    return "enhance_only" as const;
  }

  if (!!details.audioPath && canRunBatch) {
    return "batch_then_enhance" as const;
  }

  return "none" as const;
}

export function useStartListening(
  sessionId: string,
  options?: {
    liveTranscription?: boolean;
  },
) {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);

  const languages = useConfigValue("spoken_languages");

  const start = useListener((state) => state.start);
  const { conn, isLocalModel } = useSTTConnection();
  const runBatch = useRunBatch(sessionId);

  const keywords = useKeywords(sessionId);
  const requestedLiveTranscription =
    options?.liveTranscription ?? !isLocalModel;
  const liveTranscription = requestedLiveTranscription && !!conn;
  const runBatchRef = useRef(runBatch);
  const canRunBatchRef = useRef(canRunBatchTranscription(conn));
  runBatchRef.current = runBatch;
  canRunBatchRef.current = canRunBatchTranscription(conn);

  const startListening = useCallback(async () => {
    if (!store) {
      return;
    }

    let transcriptId: string | null = null;
    const startedAt = Date.now();
    const memoMd = store.getCell("sessions", sessionId, "raw_md");
    const createdAt = new Date().toISOString();

    const onStopped: OnStoppedCallback = async (_sessionId, details) => {
      const words =
        transcriptId == null ? [] : parseTranscriptWords(store, transcriptId);

      if (
        details.durationSeconds < MIN_DURATION_SECONDS &&
        words.length < MIN_WORD_COUNT
      ) {
        store.transaction(() => {
          if (transcriptId) {
            store.delRow("transcripts", transcriptId);
          }

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

      const postCaptureAction = getPostCaptureAction(
        details,
        canRunBatchRef.current,
      );

      if (postCaptureAction === "batch_then_enhance") {
        try {
          await runBatchRef.current(details.audioPath!);
        } catch (error) {
          if (isStoppedTranscriptionError(error)) {
            return;
          }
          console.error(
            "[listener] failed to run post-capture transcription",
            error,
          );
          return;
        }
      }

      if (postCaptureAction === "none") {
        return;
      }

      getEnhancerService()?.queueAutoEnhance(sessionId);
    };

    const handlePersist: LiveTranscriptPersistCallback = (delta) => {
      if (delta.new_words.length === 0 && delta.replaced_ids.length === 0) {
        return;
      }

      if (!transcriptId) {
        transcriptId = id();
        const transcriptRow = {
          session_id: sessionId,
          user_id: user_id ?? "",
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
    };

    const participantHumanIds: string[] = [];
    store.forEachRow(
      "mapping_session_participant",
      (mappingId, _forEachCell) => {
        const sid = store.getCell(
          "mapping_session_participant",
          mappingId,
          "session_id",
        );
        if (sid !== sessionId) return;
        const hid = store.getCell(
          "mapping_session_participant",
          mappingId,
          "human_id",
        );
        if (typeof hid === "string" && hid) {
          participantHumanIds.push(hid);
        }
      },
    );

    const started = await start(
      {
        session_id: sessionId,
        languages,
        onboarding: false,
        live_transcription: liveTranscription,
        model: conn?.model ?? "",
        base_url: conn?.baseUrl ?? "",
        api_key: conn?.apiKey ?? "",
        keywords,
        participant_human_ids: participantHumanIds,
        self_human_id: typeof user_id === "string" ? user_id : null,
      },
      {
        handlePersist,
        onStopped,
      },
    );

    if (!started) {
      if (transcriptId) {
        store.delRow("transcripts", transcriptId);
      }
      return;
    }

    void analyticsCommands.event({
      event: "session_started",
      has_calendar_event: !!getSessionEventById(store, sessionId),
      ...(conn
        ? {
            stt_provider: conn.provider,
            stt_model: conn.model,
          }
        : {}),
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
    liveTranscription,
  ]);

  return startListening;
}
