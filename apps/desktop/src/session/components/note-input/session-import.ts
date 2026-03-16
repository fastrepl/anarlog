import { useMutation, useQueryClient } from "@tanstack/react-query";
import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as listener2Commands } from "@hypr/plugin-listener2";
import type { TranscriptStorage } from "@hypr/store";

import { getEnhancerService } from "~/services/enhancer";
import { useListenButtonState } from "~/session/components/shared";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { ChannelProfile } from "~/stt/segment";
import { useRunBatch } from "~/stt/useRunBatch";

const AUDIO_FILE_EXTENSIONS = ["wav", "mp3", "ogg", "mp4", "m4a", "flac"];
const TRANSCRIPT_FILE_EXTENSIONS = ["vtt", "srt"];

type ImportKind = "audio" | "transcript";

function isSupportedPath(kind: ImportKind, path: string) {
  const normalizedPath = path.toLowerCase();
  const extensions =
    kind === "audio" ? AUDIO_FILE_EXTENSIONS : TRANSCRIPT_FILE_EXTENSIONS;

  return extensions.some((extension) =>
    normalizedPath.endsWith(`.${extension}`),
  );
}

export async function selectImportFile(
  kind: ImportKind,
): Promise<string | null> {
  const selection = await selectFile({
    title: kind === "audio" ? "Upload Audio" : "Upload Transcript",
    multiple: false,
    directory: false,
    defaultPath: await downloadDir(),
    filters: [
      {
        name: kind === "audio" ? "Audio" : "Transcript",
        extensions:
          kind === "audio" ? AUDIO_FILE_EXTENSIONS : TRANSCRIPT_FILE_EXTENSIONS,
      },
    ],
  });

  if (!selection) {
    return null;
  }

  return Array.isArray(selection) ? (selection[0] ?? null) : selection;
}

function useSessionTab(sessionId: string) {
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const sessionTab = useTabs((state) => {
    const found = state.tabs.find(
      (tab): tab is Extract<Tab, { type: "sessions" }> =>
        tab.type === "sessions" && tab.id === sessionId,
    );
    return found ?? null;
  });

  const openTranscriptView = useCallback(() => {
    if (!sessionTab) {
      return;
    }

    updateSessionTabState(sessionTab, {
      ...sessionTab.state,
      view: { type: "transcript" },
    });
  }, [sessionTab, updateSessionTabState]);

  return { openTranscriptView };
}

function useTriggerEnhance(sessionId: string) {
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const sessionTab = useTabs((state) => {
    const found = state.tabs.find(
      (tab): tab is Extract<Tab, { type: "sessions" }> =>
        tab.type === "sessions" && tab.id === sessionId,
    );
    return found ?? null;
  });

  return useCallback(() => {
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
}

export function useImportAudioToTranscript(sessionId: string) {
  const queryClient = useQueryClient();
  const runBatch = useRunBatch(sessionId);
  const handleBatchStarted = useListener((state) => state.handleBatchStarted);
  const handleBatchFailed = useListener((state) => state.handleBatchFailed);
  const clearBatchSession = useListener((state) => state.clearBatchSession);
  const { openTranscriptView } = useSessionTab(sessionId);
  const triggerEnhance = useTriggerEnhance(sessionId);

  const mutation = useMutation({
    mutationFn: async (path: string) => {
      if (!isSupportedPath("audio", path)) {
        throw new Error("Unsupported audio file");
      }

      openTranscriptView();
      handleBatchStarted(sessionId);

      try {
        const result = await fsSyncCommands.audioImport(sessionId, path);
        if (result.status === "error") {
          throw new Error(result.error);
        }

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

        clearBatchSession(sessionId);
        await runBatch(result.data);
        triggerEnhance();
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        handleBatchFailed(sessionId, message);
        throw error;
      }
    },
  });

  return {
    importAudio: mutation.mutateAsync,
    isImportingAudio: mutation.isPending,
  };
}

export function useImportTranscriptToSession(sessionId: string) {
  const store = main.UI.useStore(main.STORE_ID) as main.Store | undefined;
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const { openTranscriptView } = useSessionTab(sessionId);
  const triggerEnhance = useTriggerEnhance(sessionId);

  const mutation = useMutation({
    mutationFn: async (path: string) => {
      if (!isSupportedPath("transcript", path)) {
        throw new Error("Unsupported transcript file");
      }

      const result = await listener2Commands.parseSubtitle(path);
      if (result.status === "error") {
        throw new Error(String(result.error));
      }

      if (!store || result.data.tokens.length === 0) {
        return;
      }

      openTranscriptView();

      const transcriptId = crypto.randomUUID();
      const createdAt = new Date().toISOString();
      const memoMd = store.getCell("sessions", sessionId, "raw_md");

      const words = result.data.tokens.map((token) => ({
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
        token_count: result.data.tokens.length,
      });

      triggerEnhance();
    },
  });

  return {
    importTranscript: mutation.mutateAsync,
    isImportingTranscript: mutation.isPending,
  };
}

export function useSessionImportPickerActions(sessionId: string) {
  const { isDisabled: disableAudioImport, warningMessage } =
    useListenButtonState(sessionId);
  const { importAudio, isImportingAudio } =
    useImportAudioToTranscript(sessionId);
  const { importTranscript, isImportingTranscript } =
    useImportTranscriptToSession(sessionId);

  const selectAndImportAudio = useCallback(async () => {
    if (disableAudioImport) {
      return;
    }

    const path = await selectImportFile("audio");
    if (!path) {
      return;
    }

    await importAudio(path);
  }, [disableAudioImport, importAudio]);

  const selectAndImportTranscript = useCallback(async () => {
    const path = await selectImportFile("transcript");
    if (!path) {
      return;
    }

    await importTranscript(path);
  }, [importTranscript]);

  return {
    disableAudioImport,
    audioImportWarningMessage: warningMessage,
    isImportingAudio,
    isImportingTranscript,
    selectAndImportAudio,
    selectAndImportTranscript,
  };
}
