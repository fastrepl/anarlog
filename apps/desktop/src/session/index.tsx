import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { StickyNoteIcon } from "lucide-react";
import React, { useCallback, useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { useSessionBottomAccessory } from "./components/bottom-accessory";
import { CaretPositionProvider } from "./components/caret-position-context";
import { FloatingActionButton } from "./components/floating";
import { NoteInput, type NoteInputHandle } from "./components/note-input";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { SessionPreviewCard } from "./components/session-preview-card";
import { SessionSurface } from "./components/session-surface";
import { useCurrentNoteTab, useHasTranscript } from "./components/shared";
import { TitleInput, type TitleInputHandle } from "./components/title-input";
import { useAutoEnhance } from "./hooks/useAutoEnhance";
import { useIsSessionEnhancing } from "./hooks/useEnhancedNotes";
import { getSessionTabStatus } from "./tab-visual-state";

import { useTitleGeneration } from "~/ai/hooks";
import * as AudioPlayer from "~/audio-player";
import { useSessionStatusBanner } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import * as main from "~/store/tinybase/store/main";
import { useSessionTitle } from "~/store/zustand/live-title";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { consumePendingUpload } from "~/stt/pending-upload";
import { useStartListening } from "~/stt/useStartListening";
import { useSTTConnection } from "~/stt/useSTTConnection";
import { useUploadFile } from "~/stt/useUploadFile";

export const TabItemNote: TabItem<Extract<Tab, { type: "sessions" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
  pendingCloseConfirmationTab,
  setPendingCloseConfirmationTab,
}) => {
  const storeTitle = main.UI.useCell(
    "sessions",
    tab.id,
    "title",
    main.STORE_ID,
  );
  const title = useSessionTitle(tab.id, storeTitle as string | undefined);
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const stop = useListener((state) => state.stop);
  const degraded = useListener((state) => state.live.degraded);
  const isEnhancing = useIsSessionEnhancing(tab.id);
  const status = getSessionTabStatus(
    sessionMode,
    isEnhancing,
    !!degraded,
    tab.active,
  );
  const isActive =
    status === "listening" ||
    status === "listening-degraded" ||
    status === "finalizing";

  const showCloseConfirmation =
    pendingCloseConfirmationTab?.type === "sessions" &&
    pendingCloseConfirmationTab?.id === tab.id;

  const handleCloseConfirmationChange = (show: boolean) => {
    if (!show) {
      setPendingCloseConfirmationTab?.(null);
    }
  };

  const handleCloseWithStop = useCallback(() => {
    if (isActive) {
      stop();
    }
    handleCloseThis(tab);
  }, [isActive, stop, tab, handleCloseThis]);

  return (
    <SessionPreviewCard sessionId={tab.id} side="bottom" enabled={!tab.active}>
      <TabItemBase
        icon={<StickyNoteIcon className="h-4 w-4" />}
        title={title || "Untitled"}
        selected={tab.active}
        status={status}
        pinned={tab.pinned}
        tabIndex={tabIndex}
        showCloseConfirmation={showCloseConfirmation}
        onCloseConfirmationChange={handleCloseConfirmationChange}
        handleCloseThis={handleCloseWithStop}
        handleSelectThis={() => handleSelectThis(tab)}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={() => handlePinThis(tab)}
        handleUnpinThis={() => handleUnpinThis(tab)}
      />
    </SessionPreviewCard>
  );
};

export function TabContentNote({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const listenerStatus = useListener((state) => state.live.status);
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

    if (listenerStatus !== "inactive") {
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
    listenerStatus,
    conn,
    startListening,
    updateSessionTabState,
  ]);

  const { data: audioUrl } = useQuery({
    enabled: listenerStatus === "inactive",
    queryKey: ["audio", tab.id, "url"],
    queryFn: () => fsSyncCommands.audioPath(tab.id),
    select: (result) => {
      if (result.status === "error") {
        return null;
      }
      return convertFileSrc(result.data);
    },
  });

  return (
    <CaretPositionProvider>
      <SearchProvider>
        <AudioPlayer.Provider sessionId={tab.id} url={audioUrl ?? ""}>
          <TabContentNoteInner tab={tab} audioUrl={audioUrl} />
        </AudioPlayer.Provider>
      </SearchProvider>
    </CaretPositionProvider>
  );
}

function TabContentNoteInner({
  tab,
  audioUrl,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  audioUrl: string | null | undefined;
}) {
  const titleInputRef = React.useRef<TitleInputHandle>(null);
  const noteInputRef = React.useRef<NoteInputHandle>(null);

  const currentView = useCurrentNoteTab(tab);
  const { generateTitle } = useTitleGeneration(tab);
  const hasTranscript = useHasTranscript(tab.id);

  const sessionId = tab.id;
  const { skipReason } = useAutoEnhance(tab);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));

  useAutoFocusTitle({ sessionId, titleInputRef });
  usePendingUpload(sessionId);

  const { bottomAccessory, bottomAccessoryState } = useSessionBottomAccessory({
    sessionId,
    sessionMode,
    audioUrl,
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

  useSessionStatusBanner({
    skipReason,
    bottomAccessoryState,
  });

  return (
    <SessionSurface
      header={<OuterHeader sessionId={tab.id} currentView={currentView} />}
      title={
        <TitleInput
          ref={titleInputRef}
          tab={tab}
          onTransferContentToEditor={handleTransferContentToEditor}
          onFocusEditorAtStart={handleFocusEditorAtStart}
          onFocusEditorAtPixelWidth={handleFocusEditorAtPixelWidth}
          onGenerateTitle={hasTranscript ? generateTitle : undefined}
        />
      }
      afterBorder={bottomAccessory}
      floatingButton={<FloatingActionButton tab={tab} />}
    >
      <NoteInput
        ref={noteInputRef}
        tab={tab}
        onNavigateToTitle={handleNavigateToTitle}
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
