import { useQueryClient } from "@tanstack/react-query";
import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { Effect, pipe } from "effect";
import { EllipsisVerticalIcon } from "lucide-react";
import { useCallback, useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as listener2Commands } from "@hypr/plugin-listener2";
import type { TranscriptStorage } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { ActionableTooltipContent } from "./shared";

import { getEnhancerService } from "~/services/enhancer";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import type { Tab } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { fromResult } from "~/stt/fromResult";
import { ChannelProfile } from "~/stt/segment";
import { useRunBatch } from "~/stt/useRunBatch";

type FileSelection = string | string[] | null;

export function OptionsMenu({
  sessionId,
  disabled,
  warningMessage,
  onConfigure,
  children,
}: {
  sessionId: string;
  disabled: boolean;
  warningMessage: string;
  onConfigure?: () => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
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
    if (result?.type === "no_model") {
      console.warn("[enhance] skipped: no model configured");
    }
  }, [sessionId, sessionTab, updateSessionTabState]);

  const handleFilePath = useCallback(
    (selection: FileSelection, kind: "audio" | "transcript") => {
      if (!selection) {
        return Effect.void;
      }

      const path = Array.isArray(selection) ? selection[0] : selection;

      if (!path) {
        return Effect.void;
      }

      const normalizedPath = path.toLowerCase();

      if (kind === "transcript") {
        if (
          !normalizedPath.endsWith(".vtt") &&
          !normalizedPath.endsWith(".srt")
        ) {
          return Effect.void;
        }

        return pipe(
          fromResult(listener2Commands.parseSubtitle(path)),
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
      }

      if (
        !normalizedPath.endsWith(".wav") &&
        !normalizedPath.endsWith(".mp3") &&
        !normalizedPath.endsWith(".ogg") &&
        !normalizedPath.endsWith(".mp4") &&
        !normalizedPath.endsWith(".m4a") &&
        !normalizedPath.endsWith(".flac")
      ) {
        return Effect.void;
      }

      return pipe(
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
          fromResult(fsSyncCommands.audioImport(sessionId, path)),
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

  const selectAndHandleFile = useCallback(
    (
      options: {
        title: string;
        filters: { name: string; extensions: string[] }[];
      },
      kind: "audio" | "transcript",
    ) => {
      if (disabled) {
        return;
      }

      setOpen(false);

      const program = pipe(
        Effect.promise(() => downloadDir()),
        Effect.flatMap((defaultPath) =>
          Effect.promise(() =>
            selectFile({
              title: options.title,
              multiple: false,
              directory: false,
              defaultPath,
              filters: options.filters,
            }),
          ),
        ),
        Effect.flatMap((selection) => handleFilePath(selection, kind)),
      );

      Effect.runPromise(program).catch((error) => {
        console.error("[batch] failed:", error);
      });
    },
    [disabled, handleFilePath, setOpen],
  );

  const handleUploadAudio = useCallback(() => {
    if (disabled) {
      return;
    }

    selectAndHandleFile(
      {
        title: "Upload Audio",
        filters: [
          {
            name: "Audio",
            extensions: ["wav", "mp3", "ogg", "mp4", "m4a", "flac"],
          },
        ],
      },
      "audio",
    );
  }, [disabled, selectAndHandleFile]);

  const handleUploadTranscript = useCallback(() => {
    if (disabled) {
      return;
    }

    selectAndHandleFile(
      {
        title: "Upload Transcript",
        filters: [{ name: "Transcript", extensions: ["vtt", "srt"] }],
      },
      "transcript",
    );
  }, [disabled, selectAndHandleFile]);

  const moreButton = (
    <button
      className="absolute top-1/2 right-2 z-10 -translate-y-1/2 cursor-pointer text-white/70 transition-colors hover:text-white disabled:opacity-50"
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        setOpen(true);
      }}
    >
      <EllipsisVerticalIcon className="size-4" />
      <span className="sr-only">More options</span>
    </button>
  );

  if (disabled && warningMessage) {
    return (
      <div className="relative flex items-center">
        {children}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span className="inline-block">{moreButton}</span>
          </TooltipTrigger>
          <TooltipContent side="top" align="end">
            <ActionableTooltipContent
              message={warningMessage}
              action={
                onConfigure
                  ? {
                      label: "Configure",
                      handleClick: onConfigure,
                    }
                  : undefined
              }
            />
          </TooltipContent>
        </Tooltip>
      </div>
    );
  }

  if (disabled) {
    return (
      <div className="relative flex items-center">
        {children}
        {moreButton}
      </div>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div className="relative flex items-center">
          {children}
          {moreButton}
        </div>
      </PopoverTrigger>
      <PopoverContent
        side="top"
        align="center"
        sideOffset={8}
        className="w-43 rounded-xl p-1.5"
      >
        <div className="flex flex-col gap-1">
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            onClick={handleUploadAudio}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            onClick={handleUploadTranscript}
          >
            <span className="text-sm">Upload transcript</span>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
