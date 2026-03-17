import { useQueryClient } from "@tanstack/react-query";
import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { Effect, pipe } from "effect";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as listener2Commands } from "@hypr/plugin-listener2";
import type { TranscriptStorage } from "@hypr/store";

import { useListener } from "./contexts";
import { fromResult } from "./fromResult";
import { ChannelProfile } from "./segment";
import { useRunBatch } from "./useRunBatch";

import { getEnhancerService } from "~/services/enhancer";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const AUDIO_EXTENSIONS = ["wav", "mp3", "ogg", "mp4", "m4a", "flac"];
const TRANSCRIPT_EXTENSIONS = ["vtt", "srt"];

export function useUploadFile(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const queryClient = useQueryClient();
  const handleBatchStarted = useListener((state) => state.handleBatchStarted);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);
  const clearBatchSession = useListener((state) => state.clearBatchSession);

  const store = main.UI.useStore(main.STORE_ID) as main.Store | undefined;
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const sessionTab = useTabs((state) => {
    const found = state.tabs.find(
      (tab): tab is Extract<Tab, { type: "sessions" }> =>
        tab.type === "sessions" && tab.id === sessionId,
    );
    return found ?? null;
  });

  const triggerEnhance = useCallback(() => {
    const result = getEnhancerService()?.enhance(sessionId);
    if (
      (result?.type === "started" || result?.type === "already_active") &&
      sessionTab
    ) {
      updateSessionTabState(sessionTab, {
        ...sessionTab.state,
        view: { type: "enhanced", id: result.noteId },
      });
    }
  }, [sessionId, sessionTab, updateSessionTabState]);

  const processFile = useCallback(
    (filePath: string, kind: "audio" | "transcript") => {
      const normalizedPath = filePath.toLowerCase();

      if (kind === "transcript") {
        if (
          !normalizedPath.endsWith(".vtt") &&
          !normalizedPath.endsWith(".srt")
        ) {
          return;
        }

        const program = pipe(
          fromResult(listener2Commands.parseSubtitle(filePath)),
          Effect.tap((subtitle) =>
            Effect.sync(() => {
              if (!store || subtitle.tokens.length === 0) {
                return;
              }

              if (sessionTab) {
                updateSessionTabState(sessionTab, {
                  ...sessionTab.state,
                  view: { type: "transcript" },
                });
              }

              const transcriptId = crypto.randomUUID();
              const createdAt = new Date().toISOString();
              const memoMd = store.getCell("sessions", sessionId, "raw_md");

              const words = subtitle.tokens.map((token) => ({
                id: crypto.randomUUID(),
                transcript_id: transcriptId,
                text: token.text,
                start_ms: token.start_time,
                end_ms: token.end_time,
                channel: ChannelProfile.MixedCapture,
                user_id: user_id ?? "",
                created_at: new Date().toISOString(),
              }));

              const transcriptRow = {
                session_id: sessionId,
                user_id: user_id ?? "",
                created_at: createdAt,
                started_at: Date.now(),
                words: JSON.stringify(words),
                speaker_hints: "[]",
                memo_md: typeof memoMd === "string" ? memoMd : "",
              } satisfies TranscriptStorage;

              store.setRow("transcripts", transcriptId, transcriptRow);

              void analyticsCommands.event({
                event: "file_uploaded",
                file_type: "transcript",
                token_count: subtitle.tokens.length,
              });

              triggerEnhance();
            }),
          ),
        );

        Effect.runPromise(program).catch((error) => {
          console.error("[upload] transcript failed:", error);
        });
        return;
      }

      if (
        !normalizedPath.endsWith(".wav") &&
        !normalizedPath.endsWith(".mp3") &&
        !normalizedPath.endsWith(".ogg") &&
        !normalizedPath.endsWith(".mp4") &&
        !normalizedPath.endsWith(".m4a") &&
        !normalizedPath.endsWith(".flac")
      ) {
        return;
      }

      const program = pipe(
        Effect.sync(() => {
          if (sessionTab) {
            updateSessionTabState(sessionTab, {
              ...sessionTab.state,
              view: { type: "transcript" },
            });
          }
          handleBatchStarted(sessionId);
        }),
        Effect.flatMap(() =>
          fromResult(fsSyncCommands.audioImport(sessionId, filePath)),
        ),
        Effect.tap(() =>
          Effect.sync(() => {
            void analyticsCommands.event({
              event: "file_uploaded",
              file_type: "audio",
            });
            void queryClient.invalidateQueries({
              queryKey: ["audio", sessionId, "exist"],
            });
            void queryClient.invalidateQueries({
              queryKey: ["audio", sessionId, "url"],
            });
          }),
        ),
        Effect.tap(() => Effect.sync(() => clearBatchSession(sessionId))),
        Effect.flatMap((importedPath) =>
          Effect.tryPromise({
            try: () => runBatch(importedPath),
            catch: (error) => error,
          }),
        ),
        Effect.tap(() => Effect.sync(() => triggerEnhance())),
        Effect.catchAll((error: unknown) =>
          Effect.sync(() => {
            const msg = error instanceof Error ? error.message : String(error);
            handleBatchFailed(sessionId, msg);
          }),
        ),
      );

      Effect.runPromise(program).catch((error) => {
        console.error("[upload] audio failed:", error);
      });
    },
    [
      clearBatchSession,
      handleBatchFailed,
      handleBatchStarted,
      queryClient,
      runBatch,
      sessionId,
      sessionTab,
      store,
      triggerEnhance,
      updateSessionTabState,
      user_id,
    ],
  );

  const selectAndUpload = useCallback(
    (kind: "audio" | "transcript") => {
      const filters =
        kind === "audio"
          ? [{ name: "Audio", extensions: AUDIO_EXTENSIONS }]
          : [{ name: "Transcript", extensions: TRANSCRIPT_EXTENSIONS }];

      const program = pipe(
        Effect.promise(() => downloadDir()),
        Effect.flatMap((defaultPath) =>
          Effect.promise(() =>
            selectFile({
              title: kind === "audio" ? "Upload Audio" : "Upload Transcript",
              multiple: false,
              directory: false,
              defaultPath,
              filters,
            }),
          ),
        ),
      );

      Effect.runPromise(program)
        .then((selection) => {
          const path = Array.isArray(selection) ? selection[0] : selection;
          if (path) {
            processFile(path, kind);
          }
        })
        .catch((error) => {
          console.error("[upload] dialog failed:", error);
        });
    },
    [processFile],
  );

  const uploadAudio = useCallback(
    () => selectAndUpload("audio"),
    [selectAndUpload],
  );
  const uploadTranscript = useCallback(
    () => selectAndUpload("transcript"),
    [selectAndUpload],
  );

  return { uploadAudio, uploadTranscript, processFile };
}
