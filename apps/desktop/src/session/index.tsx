import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { useSessionBottomAccessory } from "./components/bottom-accessory";
import { CaretPositionProvider } from "./components/caret-position-context";
import { FloatingActionButton } from "./components/floating";
import { NoteInput, type NoteInputHandle } from "./components/note-input";
import {
  Header as NoteInputHeader,
  useEditorTabs,
} from "./components/note-input/header";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { SessionSurface } from "./components/session-surface";
import { useCurrentNoteTab, useHasTranscript } from "./components/shared";
import { NoteTitleBreadcrumb } from "./components/title-breadcrumb";
import { TitleInput, type TitleInputHandle } from "./components/title-input";
import { useAutoEnhance } from "./hooks/useAutoEnhance";

import * as AudioPlayer from "~/audio-player";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";
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
  const titleInputRef = React.useRef<TitleInputHandle>(null);
  const noteInputRef = React.useRef<NoteInputHandle>(null);

  const editorTabs = useEditorTabs({ sessionId: tab.id });
  const currentView = useCurrentNoteTab(tab);
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const hasTranscript = useHasTranscript(tab.id);

  const sessionId = tab.id;
  const { skipReason } = useAutoEnhance(tab);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const { audioExists } = AudioPlayer.useAudioPlayer();

  useAutoFocusTitle({ sessionId, titleInputRef });
  usePendingUpload(sessionId);

  const { bottomAccessory, bottomBorderHandle, bottomAccessoryState } =
    useSessionBottomAccessory({
      sessionId,
      sessionMode,
      audioExists,
      audioUrlReady,
      isAudioLoading: isAudioUrlLoading,
      hasTranscript,
      suppressTranscriptPanel: currentView.type === "transcript",
    });

  const handleTabChange = React.useCallback(
    (view: typeof currentView) => {
      noteInputRef.current?.prepareForTabChange();
      updateSessionTabState(tab, { ...tab.state, view });
    },
    [tab, updateSessionTabState],
  );
  const handleNavigateToTitle = React.useCallback((pixelWidth?: number) => {
    if (pixelWidth !== undefined) {
      titleInputRef.current?.focusAtPixelWidth(pixelWidth);
    } else {
      titleInputRef.current?.focusAtEnd();
    }
  }, []);
  const handleTransferContentToEditor = React.useCallback((content: string) => {
    noteInputRef.current?.insertAtStartAndFocus(content);
  }, []);
  const handleFocusEditorAtStart = React.useCallback(() => {
    noteInputRef.current?.focusAtStart();
  }, []);
  const handleFocusEditorAtPixelWidth = React.useCallback(
    (pixelWidth: number) => {
      noteInputRef.current?.focusAtPixelWidth(pixelWidth);
    },
    [],
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
  const showBottomAccessory =
    currentView.type !== "transcript" ||
    bottomAccessoryState?.mode === "playback";
  const showBottomTranscriptSurface =
    showBottomAccessory && currentView.type !== "transcript";

  return (
    <SessionSurface
      header={
        <OuterHeader
          sessionId={tab.id}
          currentView={currentView}
          standaloneWindow={standaloneWindow}
          title={
            <div className="flex min-w-0 items-center gap-2">
              <div className="min-w-0 flex-1">
                <NoteTitleBreadcrumb
                  sessionId={tab.id}
                  title={
                    <TitleInput
                      ref={titleInputRef}
                      tab={tab}
                      variant="breadcrumb"
                      onTransferContentToEditor={handleTransferContentToEditor}
                      onFocusEditorAtStart={handleFocusEditorAtStart}
                      onFocusEditorAtPixelWidth={handleFocusEditorAtPixelWidth}
                    />
                  }
                />
              </div>
              <div className="shrink-0">
                <NoteInputHeader
                  sessionId={tab.id}
                  editorTabs={editorTabs}
                  currentTab={currentView}
                  handleTabChange={handleTabChange}
                />
              </div>
            </div>
          }
        />
      }
      afterBorder={showBottomAccessory ? bottomAccessory : null}
      afterBorderExpanded={
        showBottomTranscriptSurface && resizeTranscriptSurface
      }
      afterBorderFlush={
        showBottomTranscriptSurface && bottomAccessoryState?.mode === "live"
      }
      afterBorderResizable={
        showBottomTranscriptSurface && canResizeTranscriptSurface
      }
      bottomBorderHandle={showBottomAccessory ? bottomBorderHandle : null}
      mergeAfterBorder={showBottomTranscriptSurface && mergeTranscriptSurface}
      floatingButton={
        <FloatingActionButton
          allowListening={!standaloneWindow}
          skipReason={skipReason}
          tab={tab}
        />
      }
    >
      <NoteInput
        ref={noteInputRef}
        tab={tab}
        onNavigateToTitle={handleNavigateToTitle}
        editorTabs={editorTabs}
        currentTab={currentView}
        handleTabChange={handleTabChange}
        hideHeader
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

function useAutoFocusTitle({
  sessionId,
  titleInputRef,
}: {
  sessionId: string;
  titleInputRef: React.RefObject<TitleInputHandle | null>;
}) {
  const didAutoFocus = useRef(false);
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);

  useEffect(() => {
    if (didAutoFocus.current) return;

    if (!title) {
      titleInputRef.current?.focus();
      didAutoFocus.current = true;
    }
  }, [sessionId, title]);
}
