import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { useSessionBottomAccessory } from "./components/bottom-accessory";
import { CaretPositionProvider } from "./components/caret-position-context";
import { getPersistedNoteTabView } from "./components/compute-note-tab";
import { FloatingActionButton } from "./components/floating";
import { NoteInput, type NoteInputHandle } from "./components/note-input";
import { Header, useEditorTabs } from "./components/note-input/header";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { SessionSurface } from "./components/session-surface";
import { useCurrentNoteTab, useHasTranscript } from "./components/shared";
import { useAutoEnhance } from "./hooks/useAutoEnhance";

import * as AudioPlayer from "~/audio-player";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { consumePendingUpload } from "~/stt/pending-upload";
import { useStartListening } from "~/stt/useStartListening";
import { useSTTConnection } from "~/stt/useSTTConnection";
import { useUploadFile } from "~/stt/useUploadFile";

export function TabContentNote({
  standaloneWindow = false,
  tab,
}: {
  standaloneWindow?: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const canStartLiveSession = useListener((state) =>
    state.canStartLiveSession(tab.id),
  );
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const { conn } = useSTTConnection();
  const startListening = useStartListening(tab.id);
  const hasAttemptedAutoStart = useRef(false);

  useEffect(() => {
    if (!tab.state.autoStart) {
      hasAttemptedAutoStart.current = false;
      return;
    }

    if (standaloneWindow) {
      return;
    }

    if (hasAttemptedAutoStart.current) {
      return;
    }

    if (!canStartLiveSession) {
      return;
    }

    if (!conn) {
      return;
    }

    hasAttemptedAutoStart.current = true;
    startListening();
    updateSessionTabState(tab, { ...tab.state, autoStart: null });
  }, [
    tab.id,
    tab.state,
    tab.state.autoStart,
    standaloneWindow,
    canStartLiveSession,
    conn,
    startListening,
    updateSessionTabState,
  ]);

  const audioUrlQuery = useQuery({
    enabled: sessionMode !== "active" && sessionMode !== "finalizing",
    queryKey: ["audio", tab.id, "url"],
    queryFn: () => fsSyncCommands.audioPath(tab.id),
    select: (result) => {
      if (result.status === "error") {
        return null;
      }
      return convertFileSrc(result.data);
    },
  });
  const audioUrl = audioUrlQuery.data;

  return (
    <CaretPositionProvider>
      <SearchProvider>
        <AudioPlayer.Provider sessionId={tab.id} url={audioUrl ?? ""}>
          <TabContentNoteInner
            tab={tab}
            standaloneWindow={standaloneWindow}
            audioUrlReady={Boolean(audioUrl)}
            isAudioUrlLoading={audioUrlQuery.isPending}
          />
        </AudioPlayer.Provider>
      </SearchProvider>
    </CaretPositionProvider>
  );
}

function TabContentNoteInner({
  tab,
  standaloneWindow,
  audioUrlReady,
  isAudioUrlLoading,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  standaloneWindow: boolean;
  audioUrlReady: boolean;
  isAudioUrlLoading: boolean;
}) {
  const noteInputRef = React.useRef<NoteInputHandle>(null);
  const currentView = useCurrentNoteTab(tab);
  const editorTabs = useEditorTabs({ sessionId: tab.id });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);

  const sessionId = tab.id;
  const { skipReason } = useAutoEnhance(tab);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const hasTranscript = useHasTranscript(sessionId);
  const { audioExists } = AudioPlayer.useAudioPlayer();

  usePendingUpload(sessionId);

  const { bottomAccessory, bottomBorderHandle, bottomAccessoryState } =
    useSessionBottomAccessory({
      sessionId,
      sessionMode,
      audioExists,
      audioUrlReady,
      isAudioLoading: isAudioUrlLoading,
      hasTranscript,
    });

  const handleHeaderTabChange = React.useCallback(
    (view: EditorView) => {
      noteInputRef.current?.preserveScroll();
      updateSessionTabState(tab, {
        ...tab.state,
        view: getPersistedNoteTabView(view, sessionMode === "active"),
      });
    },
    [sessionMode, tab, updateSessionTabState],
  );

  const mergeTranscriptSurface =
    bottomAccessoryState?.expanded === true &&
    (bottomAccessoryState.mode === "playback" ||
      bottomAccessoryState.mode === "transcript_only");
  const canResizeTranscriptSurface =
    bottomAccessoryState?.mode === "live" ||
    bottomAccessoryState?.mode === "playback" ||
    bottomAccessoryState?.mode === "transcript_only";
  const hasResizableTranscriptSurface =
    bottomAccessoryState?.mode === "live" ||
    bottomAccessoryState?.mode === "transcript_only" ||
    (bottomAccessoryState?.mode === "playback" &&
      (hasTranscript || sessionMode === "running_batch"));
  const resizeTranscriptSurface =
    bottomAccessoryState?.expanded === true &&
    canResizeTranscriptSurface &&
    hasResizableTranscriptSurface;

  return (
    <SessionSurface
      header={
        <OuterHeader
          sessionId={tab.id}
          currentView={currentView}
          standaloneWindow={standaloneWindow}
          title={
            <Header
              sessionId={sessionId}
              editorTabs={editorTabs}
              currentTab={currentView}
              handleTabChange={handleHeaderTabChange}
            />
          }
        />
      }
      afterBorder={bottomAccessory}
      afterBorderExpanded={resizeTranscriptSurface}
      afterBorderFlush={bottomAccessoryState?.mode === "live"}
      afterBorderResizable={canResizeTranscriptSurface}
      bottomBorderHandle={bottomBorderHandle}
      floatingButton={
        <FloatingActionButton
          allowListening={!standaloneWindow}
          skipReason={skipReason}
          tab={tab}
        />
      }
      mergeAfterBorder={mergeTranscriptSurface}
    >
      <NoteInput
        ref={noteInputRef}
        tab={tab}
        currentTab={currentView}
        editorTabs={editorTabs}
      />
    </SessionSurface>
  );
}

function usePendingUpload(sessionId: string) {
  const { processFile } = useUploadFile(sessionId);
  const processFileRef = useRef(processFile);
  processFileRef.current = processFile;

  useEffect(() => {
    const pending = consumePendingUpload(sessionId);
    if (pending) {
      processFileRef.current(pending.filePath, pending.kind);
    }
  }, [sessionId]);
}
