import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import { CaretPositionProvider } from "./components/caret-position-context";
import { FloatingActionButton } from "./components/floating";
import {
  NoteInput,
  shouldShowTranscriptTabSpinner,
  type NoteInputHandle,
} from "./components/note-input";
import {
  createEditorTabs,
  Header as NoteInputHeader,
} from "./components/note-input/header";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { SessionSurface } from "./components/session-surface";
import {
  computeCurrentNoteTab,
  getCanShowTranscript,
  useHasTranscript,
} from "./components/shared";
import { useAutoEnhance } from "./hooks/useAutoEnhance";
import { useEnsureDefaultSummaryFromState } from "./hooks/useEnhancedNotes";
import { shouldShowSessionTopAudioPlayer } from "./top-audio-player";

import * as AudioPlayer from "~/audio-player";
import { hydrateSessionContent } from "~/store/tinybase/persister/session/hydrate";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { consumePendingUpload } from "~/stt/pending-upload";
import { useStartListening } from "~/stt/useStartListening";
import { useSTTConnection } from "~/stt/useSTTConnection";
import { useUploadFile } from "~/stt/useUploadFile";

const hydratedSessionIds = new Set<string>();

export function TabContentNote({
  standaloneWindow = false,
  tab,
}: {
  standaloneWindow?: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const audioExists = AudioPlayer.useAudioExists(tab.id);

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
      {tab.state.autoStart && !standaloneWindow ? (
        <AutoStartListening tab={tab} />
      ) : null}
      <SearchProvider>
        <AudioPlayer.Provider sessionId={tab.id} url={audioUrl ?? ""}>
          <TabContentNoteInner
            tab={tab}
            standaloneWindow={standaloneWindow}
            audioUrlReady={Boolean(audioUrl)}
            audioExists={audioExists}
          />
        </AudioPlayer.Provider>
      </SearchProvider>
    </CaretPositionProvider>
  );
}

function AutoStartListening({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const canStartLiveSession = useListener((state) =>
    state.canStartLiveSession(tab.id),
  );
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const { conn } = useSTTConnection();
  const startListening = useStartListening(tab.id);
  const hasAttemptedAutoStart = useRef(false);

  useEffect(() => {
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
    tab,
    tab.state,
    canStartLiveSession,
    conn,
    startListening,
    updateSessionTabState,
  ]);

  return null;
}

function TabContentNoteInner({
  tab,
  standaloneWindow,
  audioUrlReady,
  audioExists,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  standaloneWindow: boolean;
  audioUrlReady: boolean;
  audioExists: boolean;
}) {
  const noteInputRef = React.useRef<NoteInputHandle>(null);

  const sessionId = tab.id;
  usePendingUpload(sessionId);

  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener((state) => state.batch[sessionId]?.error);
  const hasLiveSegments = useListener(
    (state) =>
      state.live.sessionId === sessionId && state.liveSegments.length > 0,
  );
  const canShowTranscript = getCanShowTranscript({
    audioExists,
    batchError: Boolean(batchError),
    hasLiveSegments,
    hasTranscript,
    sessionMode,
  });
  const enhancedNoteIds =
    main.UI.useSliceRowIds(
      main.INDEXES.enhancedNotesBySession,
      sessionId,
      main.STORE_ID,
    ) ?? [];
  const firstEnhancedNoteId = enhancedNoteIds[0];
  const contentHydrated = useHydrateSessionContent(sessionId);
  useEnsureDefaultSummaryFromState({
    batchError: Boolean(batchError),
    enabled: contentHydrated,
    enhancedNoteCount: enhancedNoteIds.length,
    hasTranscript,
    sessionId,
    sessionMode,
  });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);

  const { skipReason } = useAutoEnhance(tab);
  const isTranscribing = shouldShowTranscriptTabSpinner(sessionMode);
  const isLiveSessionActive = sessionMode === "active";
  const editorTabs = React.useMemo(
    () =>
      createEditorTabs({
        enhancedNoteIds,
        canShowTranscript,
      }),
    [enhancedNoteIds, canShowTranscript],
  );
  const currentView = React.useMemo(() => {
    return computeCurrentNoteTab(
      tab.state.view ?? null,
      isLiveSessionActive,
      firstEnhancedNoteId,
      canShowTranscript,
    );
  }, [
    tab.state.view,
    isLiveSessionActive,
    firstEnhancedNoteId,
    canShowTranscript,
  ]);
  useAutoFocusTitle({ sessionId, noteInputRef });

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
  return (
    <>
      <SessionSurface
        header={
          <OuterHeader
            sessionId={sessionId}
            currentView={currentView}
            standaloneWindow={standaloneWindow}
            title={
              <NoteInputHeader
                sessionId={sessionId}
                editorTabs={editorTabs}
                currentTab={currentView}
                handleTabChange={handleTabChange}
                isTranscribing={isTranscribing}
              />
            }
          />
        }
        floatingButton={
          <FloatingActionButton
            allowListening={!standaloneWindow}
            audioExists={audioExists}
            currentView={currentView}
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
            {contentHydrated ? (
              <NoteInput
                ref={noteInputRef}
                tab={tab}
                editorTabs={editorTabs}
                currentTab={currentView}
                handleTabChange={handleTabChange}
                sessionMode={sessionMode}
                hideHeader
              />
            ) : (
              <SessionContentLoading />
            )}
          </div>
        </div>
      </SessionSurface>
    </>
  );
}

function useHydrateSessionContent(sessionId: string): boolean {
  const store = main.UI.useStore(main.STORE_ID);
  const [retryAttempt, setRetryAttempt] = React.useState(0);
  const [hydrated, setHydrated] = React.useState(() =>
    hydratedSessionIds.has(sessionId),
  );

  useEffect(() => {
    if (hydratedSessionIds.has(sessionId)) {
      setHydrated(true);
      return;
    }

    if (!store) {
      setHydrated(false);
      return;
    }

    let active = true;
    setHydrated(false);

    void hydrateSessionContent(store, sessionId).then((success) => {
      if (success) {
        hydratedSessionIds.add(sessionId);
      }
      if (active) {
        setHydrated(success);
        if (!success) {
          window.setTimeout(() => {
            if (active) {
              setRetryAttempt((attempt) => attempt + 1);
            }
          }, 1000);
        }
      }
    });

    return () => {
      active = false;
    };
  }, [store, sessionId, retryAttempt]);

  return hydrated;
}

function SessionContentLoading() {
  return (
    <div className="flex h-full flex-col gap-3 px-4 py-5">
      <div className="bg-muted h-5 w-3/5 animate-pulse rounded-md" />
      <div className="bg-muted/80 h-4 w-4/5 animate-pulse rounded-md" />
      <div className="bg-muted/70 h-4 w-2/3 animate-pulse rounded-md" />
    </div>
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
