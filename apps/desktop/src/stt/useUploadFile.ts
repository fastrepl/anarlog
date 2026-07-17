import { useQueryClient } from "@tanstack/react-query";
import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { Effect, pipe } from "effect";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import {
  commands as fsSyncCommands,
  events as fsSyncEvents,
} from "@hypr/plugin-fs-sync";
import { commands as listener2Commands } from "@hypr/plugin-transcription";

import { estimateUploadedAudioSessionCreatedAt } from "./audio-note-date";
import { useListener } from "./contexts";
import { fromResult } from "./fromResult";
import { ChannelProfile } from "./segment";
import { isStoppedTranscriptionError, useRunBatch } from "./useRunBatch";

import { getEnhancerService } from "~/services/enhancer";
import { catalogLocalSessionAudio } from "~/session/attachments";
import { enqueueSessionAudioOperation } from "~/session/audio-operations";
import { useSession, useUpdateSession } from "~/session/queries";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { createTranscript } from "~/stt/queries";

export const AUDIO_EXTENSIONS = [
  "wav",
  "mp3",
  "ogg",
  "mp4",
  "m4a",
  "flac",
  "webm",
  "aac",
];
const AUDIO_TRANSFER_EXTENSIONS = [...AUDIO_EXTENSIONS, "qta"];
const TRANSCRIPT_EXTENSIONS = ["vtt", "srt"];

function fileExtension(value: string) {
  const extension = value.toLowerCase().split(".").pop();
  return extension && extension !== value.toLowerCase() ? extension : "";
}

export function isAudioUploadFile(file: Pick<File, "name" | "type">) {
  return (
    AUDIO_TRANSFER_EXTENSIONS.includes(fileExtension(file.name)) ||
    file.type.startsWith("audio/")
  );
}

function isAudioUploadPath(path: string) {
  return AUDIO_EXTENSIONS.includes(fileExtension(path));
}

export function useUploadFile(sessionId: string) {
  const runBatch = useRunBatch(sessionId);
  const queryClient = useQueryClient();
  const handleBatchStarted = useListener((state) => state.handleBatchStarted);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);
  const updateBatchProgress = useListener((state) => state.updateBatchProgress);
  const clearBatchSession = useListener((state) => state.clearBatchSession);

  const session = useSession(sessionId);
  const updateSession = useUpdateSession(sessionId);
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const sessionTab = useTabs((state) => {
    const found = state.tabs.find(
      (tab): tab is Extract<Tab, { type: "sessions" }> =>
        tab.type === "sessions" && tab.id === sessionId,
    );
    return found ?? null;
  });

  const triggerEnhance = useCallback(() => {
    const service = getEnhancerService();
    if (!service) return;

    void Promise.resolve(service.enhance(sessionId))
      .then((result) => {
        if (
          (result.type === "started" || result.type === "already_active") &&
          sessionTab
        ) {
          updateSessionTabState(sessionTab, {
            ...sessionTab.state,
            view: { type: "enhanced", id: result.noteId },
          });
        }
      })
      .catch((error) => {
        console.error("[enhancer] failed to enhance uploaded file", error);
      });
  }, [sessionId, sessionTab, updateSessionTabState]);

  const triggerEnhanceIfSummaryEmpty = useCallback(() => {
    void Promise.resolve(
      getEnhancerService()?.queueAutoEnhanceIfSummaryEmpty(sessionId),
    ).catch((error) => {
      console.error("[enhancer] failed to queue uploaded file", error);
    });
  }, [sessionId]);

  const applyEstimatedAudioNoteDate = useCallback(
    async (filePath: string) => {
      try {
        if (session?.event_json.trim()) {
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

        await updateSession({ created_at: estimatedCreatedAt });
      } catch (error) {
        console.error("[upload] audio metadata inspection failed:", error);
      }
    },
    [session?.event_json, updateSession],
  );

  const applyDroppedAudioNoteDate = useCallback(
    async (file: File) => {
      try {
        if (session?.event_json.trim()) {
          return;
        }

        if (!Number.isFinite(file.lastModified) || file.lastModified <= 0) {
          return;
        }

        await updateSession({
          created_at: new Date(file.lastModified).toISOString(),
        });
      } catch (error) {
        console.error("[upload] dropped audio date inspection failed:", error);
      }
    },
    [session?.event_json, updateSession],
  );

  const importWithProgress = useCallback(
    async (
      runImport: () => Promise<
        | { status: "ok"; data: string }
        | {
            status: "error";
            error: string;
          }
      >,
    ) =>
      enqueueSessionAudioOperation(sessionId, async () => {
        const unlisten = await fsSyncEvents.audioImportEvent.listen((e) => {
          if (
            e.payload.type === "audioImportProgress" &&
            e.payload.session_id === sessionId
          ) {
            updateBatchProgress(sessionId, e.payload.percentage);
          }
        });

        try {
          const result = await runImport();
          if (result.status === "error") {
            throw new Error(result.error);
          }
          try {
            await catalogLocalSessionAudio(sessionId);
          } catch (error) {
            console.error("[upload] failed to catalog imported audio", error);
          }
          return result.data;
        } finally {
          unlisten();
        }
      }),
    [sessionId, updateBatchProgress],
  );

  const runAudioImport = useCallback(
    (
      importAudio: () => Promise<string>,
      inspectAudioDate: () => Promise<void>,
    ) => {
      const program = pipe(
        Effect.promise(inspectAudioDate),
        Effect.tap(() =>
          Effect.sync(() => {
            handleBatchStarted(sessionId, "importing");
          }),
        ),
        Effect.flatMap(() =>
          Effect.tryPromise({
            try: importAudio,
            catch: (error) =>
              error instanceof Error ? error : new Error(String(error)),
          }),
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
        Effect.tap(() => Effect.sync(() => triggerEnhanceIfSummaryEmpty())),
        Effect.catchAll((error: unknown) =>
          Effect.sync(() => {
            if (isStoppedTranscriptionError(error)) {
              return;
            }
            const msg = error instanceof Error ? error.message : String(error);
            console.error("[upload] audio import failed:", error);
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
      triggerEnhanceIfSummaryEmpty,
    ],
  );

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
          Effect.tap((subtitle) => {
            if (subtitle.tokens.length === 0) {
              return Effect.void;
            }

            const transcriptId = crypto.randomUUID();
            const createdAt = new Date().toISOString();

            const words = subtitle.tokens.map((token) => ({
              id: crypto.randomUUID(),
              transcript_id: transcriptId,
              text: token.text,
              start_ms: token.start_time,
              end_ms: token.end_time,
              channel: ChannelProfile.MixedCapture,
              user_id: session?.user_id ?? "",
              created_at: new Date().toISOString(),
            }));

            return Effect.tryPromise({
              try: () =>
                createTranscript({
                  id: transcriptId,
                  sessionId,
                  ownerUserId: session?.user_id ?? "",
                  createdAt,
                  startedAt: Date.now(),
                  memo: session?.raw_md ?? "",
                  source: "subtitle_import",
                  words,
                }),
              catch: (error) =>
                error instanceof Error ? error : new Error(String(error)),
            }).pipe(
              Effect.tap(() =>
                Effect.sync(() => {
                  void analyticsCommands.event({
                    event: "file_uploaded",
                    file_type: "transcript",
                    token_count: subtitle.tokens.length,
                  });

                  triggerEnhance();
                }),
              ),
            );
          }),
        );

        Effect.runPromise(program).catch((error) => {
          console.error("[upload] transcript failed:", error);
        });
        return;
      }

      if (!isAudioUploadPath(normalizedPath)) {
        return;
      }

      runAudioImport(
        () =>
          importWithProgress(() =>
            fsSyncCommands.audioImport(sessionId, filePath),
          ),
        () => applyEstimatedAudioNoteDate(filePath),
      );
    },
    [
      sessionId,
      session,
      triggerEnhance,
      applyEstimatedAudioNoteDate,
      importWithProgress,
      runAudioImport,
    ],
  );

  const processAudioFile = useCallback(
    (
      file: File,
      options?: { allowUnknownAudio?: boolean; contentType?: string },
    ) => {
      if (!options?.allowUnknownAudio && !isAudioUploadFile(file)) {
        return;
      }

      const filePath = audioUploadFilePath(file);
      runAudioImport(
        async () => {
          if (filePath) {
            return importWithProgress(() =>
              fsSyncCommands.audioImport(sessionId, filePath),
            );
          }

          const data = Array.from(new Uint8Array(await file.arrayBuffer()));
          return importWithProgress(() =>
            fsSyncCommands.audioImportData(
              sessionId,
              data,
              file.name,
              options?.contentType || file.type || null,
            ),
          );
        },
        () => applyDroppedAudioNoteDate(file),
      );
    },
    [applyDroppedAudioNoteDate, importWithProgress, runAudioImport, sessionId],
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

  return { uploadAudio, uploadTranscript, processFile, processAudioFile };
}

function audioUploadFilePath(file: File) {
  const value = (file as { path?: unknown }).path;
  return typeof value === "string" && value.trim() ? value : null;
}
