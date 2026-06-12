import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { useSessionBottomAccessory } from "./components/bottom-accessory";
import { CaretPositionProvider } from "./components/caret-position-context";
import { FloatingActionButton } from "./components/floating";
import { NoteInput, type NoteInputHandle } from "./components/note-input";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { SessionSurface } from "./components/session-surface";
import { useCurrentNoteTab, useHasTranscript } from "./components/shared";
import { TitleInput, type TitleInputHandle } from "./components/title-input";
import { getNextFloatingButtonHidden } from "./floating-scroll-state";
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
  tab,
}: {
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
  audioUrlReady,
  isAudioUrlLoading,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  audioUrlReady: boolean;
  isAudioUrlLoading: boolean;
}) {
  const titleInputRef = React.useRef<TitleInputHandle>(null);
  const noteInputRef = React.useRef<NoteInputHandle>(null);
  const noteScrollRef = React.useRef({
    viewKey: "",
    scrollTop: 0,
  });

  const currentView = useCurrentNoteTab(tab);
  const currentViewKey =
    currentView.type === "enhanced"
      ? `enhanced-${currentView.id}`
      : currentView.type;
  const hasTranscript = useHasTranscript(tab.id);
  const [floatingButtonScrollState, setFloatingButtonScrollState] =
    React.useState({
      viewKey: currentViewKey,
      hidden: false,
    });
  const floatingButtonHidden =
    floatingButtonScrollState.viewKey === currentViewKey &&
    floatingButtonScrollState.hidden;

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
    });

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

  const handleNoteScroll = React.useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      const scrollTop = event.currentTarget.scrollTop;
      const scrollHeight = event.currentTarget.scrollHeight;
      const clientHeight = event.currentTarget.clientHeight;
      const lastScroll =
        noteScrollRef.current.viewKey === currentViewKey
          ? noteScrollRef.current.scrollTop
          : scrollTop;
      const delta = scrollTop - lastScroll;

      noteScrollRef.current = {
        viewKey: currentViewKey,
        scrollTop,
      };

      setFloatingButtonScrollState((state) => {
        const hidden = getNextFloatingButtonHidden({
          currentHidden:
            state.viewKey === currentViewKey ? state.hidden : false,
          delta,
          scrollTop,
          scrollHeight,
          clientHeight,
        });

        return state.viewKey === currentViewKey && state.hidden === hidden
          ? state
          : { viewKey: currentViewKey, hidden };
      });
    },
    [currentViewKey],
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
          title={
            <TitleInput
              ref={titleInputRef}
              tab={tab}
              onTransferContentToEditor={handleTransferContentToEditor}
              onFocusEditorAtStart={handleFocusEditorAtStart}
              onFocusEditorAtPixelWidth={handleFocusEditorAtPixelWidth}
            />
          }
        />
      }
      afterBorder={bottomAccessory}
      afterBorderExpanded={resizeTranscriptSurface}
      afterBorderFlush={bottomAccessoryState?.mode === "live"}
      afterBorderResizable={canResizeTranscriptSurface}
      bottomBorderHandle={bottomBorderHandle}
      mergeAfterBorder={mergeTranscriptSurface}
      floatingButton={
        <FloatingActionButton
          hidden={floatingButtonHidden}
          skipReason={skipReason}
          tab={tab}
        />
      }
    >
      <NoteInput
        ref={noteInputRef}
        tab={tab}
        onNavigateToTitle={handleNavigateToTitle}
        onScroll={handleNoteScroll}
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
  // Prevent re-focusing when the user intentionally leaves the title empty.
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
