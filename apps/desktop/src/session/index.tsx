import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { shouldShowSessionBottomAccessory } from "./bottom-accessory-visibility";
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
import { useAutoEnhance } from "./hooks/useAutoEnhance";
import { shouldShowSessionTopAudioPlayer } from "./top-audio-player";

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
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  standaloneWindow: boolean;
  audioUrlReady: boolean;
}) {
  const noteInputRef = React.useRef<NoteInputHandle>(null);

  const { audioExists } = AudioPlayer.useAudioPlayer();
  const editorTabs = useEditorTabs({ sessionId: tab.id, audioExists });
  const currentView = useCurrentNoteTab(tab, { audioExists });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const hasTranscript = useHasTranscript(tab.id);

  const sessionId = tab.id;
  const { skipReason } = useAutoEnhance(tab);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const isTranscribing =
    sessionMode === "active" ||
    sessionMode === "finalizing" ||
    sessionMode === "running_batch";
  useAutoFocusTitle({ sessionId, noteInputRef });
  usePendingUpload(sessionId);

  const { bottomAccessory, bottomBorderHandle, bottomAccessoryState } =
    useSessionBottomAccessory({
      sessionId,
      sessionMode,
    });
  const showTopAudioPlayer = shouldShowSessionTopAudioPlayer({
    audioExists,
    audioUrlReady,
    currentView,
    sessionMode,
  });

  const handleTabChange = React.useCallback(
    (view: typeof currentView) => {
      noteInputRef.current?.prepareForTabChange();
      updateSessionTabState(tab, { ...tab.state, view });
    },
    [tab, updateSessionTabState],
  );
  const mergeTranscriptSurface =
    bottomAccessoryState?.expanded === true &&
    (bottomAccessoryState.mode === "playback" ||
      bottomAccessoryState.mode === "transcript_only");
  const canResizeTranscriptSurface =
    bottomAccessoryState?.mode === "playback" ||
    bottomAccessoryState?.mode === "transcript_only";
  const hasResizableTranscriptSurface =
    bottomAccessoryState?.mode === "transcript_only" ||
    (bottomAccessoryState?.mode === "playback" &&
      (hasTranscript || sessionMode === "running_batch"));
  const resizeTranscriptSurface =
    bottomAccessoryState?.expanded === true &&
    canResizeTranscriptSurface &&
    hasResizableTranscriptSurface;
  const showBottomAccessory = shouldShowSessionBottomAccessory({
    currentView,
    bottomAccessoryState,
    sessionMode,
  });
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
            <NoteInputHeader
              sessionId={tab.id}
              editorTabs={editorTabs}
              currentTab={currentView}
              handleTabChange={handleTabChange}
              isTranscribing={isTranscribing}
            />
          }
        />
      }
      afterBorder={showBottomAccessory ? bottomAccessory : null}
      afterBorderExpanded={
        showBottomTranscriptSurface && resizeTranscriptSurface
      }
      afterBorderResizable={
        showBottomTranscriptSurface && canResizeTranscriptSurface
      }
      bottomBorderHandle={showBottomAccessory ? bottomBorderHandle : null}
      mergeAfterBorder={showBottomTranscriptSurface && mergeTranscriptSurface}
      floatingButton={
        <FloatingActionButton
          allowListening={!standaloneWindow}
          audioExists={audioExists}
          skipReason={skipReason}
          tab={tab}
        />
      }
    >
      <div className="flex h-full min-h-0 flex-col">
        {showTopAudioPlayer ? (
          <div
            data-session-top-audio-player
            className="shrink-0 px-1 pt-1 pb-2"
          >
            <div className="border-border/70 bg-card/80 overflow-hidden rounded-[22px] border">
              <AudioPlayer.Timeline contentClassName="py-1.5 pr-3 pl-1" />
            </div>
          </div>
        ) : null}
        <div className="min-h-0 flex-1">
          <NoteInput
            ref={noteInputRef}
            tab={tab}
            editorTabs={editorTabs}
            currentTab={currentView}
            handleTabChange={handleTabChange}
            hideHeader
          />
        </div>
      </div>
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
  noteInputRef,
}: {
  sessionId: string;
  noteInputRef: React.RefObject<NoteInputHandle | null>;
}) {
  const autoFocusedSessionRef = useRef<string | null>(null);
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);

  useEffect(() => {
    if (autoFocusedSessionRef.current === sessionId) return;

    if (!title) {
      noteInputRef.current?.focusAtStart();
      autoFocusedSessionRef.current = sessionId;
    }
  }, [sessionId, title]);
}
