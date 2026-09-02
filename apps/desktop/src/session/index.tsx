import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import React, { useEffect, useRef } from "react";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { FloatingActionButton } from "./components/floating";
import {
  NoteInput,
  shouldShowTranscriptTabSpinner,
  type NoteInputHandle,
} from "./components/note-input";
import {
  createEditorTabs,
  SessionViewSwitcher,
} from "./components/note-input/header";
import { SearchProvider } from "./components/note-input/search/context";
import { OuterHeader } from "./components/outer-header";
import { PendingProposalsBanner } from "./components/pending-proposals-banner";
import { SessionSurface } from "./components/session-surface";
import {
  computeCurrentNoteTab,
  getCanShowTranscript,
  useHasTranscript,
} from "./components/shared";
import { useAutoEnhance } from "./hooks/useAutoEnhance";
import {
  useEnhancedNotes,
  useEnsureDefaultSummaryFromState,
} from "./hooks/useEnhancedNotes";
import { shouldShowSessionTopAudioPlayer } from "./top-audio-player";
import { getSessionEvent } from "./utils";

import * as AudioPlayer from "~/audio-player";
import { isLockedFlag } from "~/lock/flag";
import { revealLockedNote } from "~/lock/notes";
import { NoteLockScreen } from "~/lock/screen";
import { useAppLock } from "~/lock/store";
import {
  isCanonicalSessionImportLocked,
  subscribeCanonicalSessionImportLocks,
} from "~/session-sharing/editor-activity";
import { useSession } from "~/session/queries";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { consumePendingUpload } from "~/stt/pending-upload";
import { ScheduledSessionAutoStart } from "~/stt/scheduled-session-auto-start";
import { useUploadFile } from "~/stt/useUploadFile";

export function TabContentNote({
  standaloneWindow = false,
  tab,
}: {
  standaloneWindow?: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const importLocked = React.useSyncExternalStore(
    subscribeCanonicalSessionImportLocks,
    () => isCanonicalSessionImportLocked(tab.id),
    () => isCanonicalSessionImportLocked(tab.id),
  );

  if (importLocked) return <SessionContentLoading />;

  return <LockedNoteGate tab={tab} standaloneWindow={standaloneWindow} />;
}

function LockedNoteGate({
  standaloneWindow,
  tab,
}: {
  standaloneWindow: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const session = useSession(tab.id);
  const locked = isLockedFlag(session?.locked);
  const revealed = useAppLock((state) =>
    Boolean(state.revealedNoteIds[tab.id]),
  );
  const authenticating = useAppLock((state) => state.authenticating);
  const lockOverlay =
    session && locked && !revealed ? (
      <NoteLockScreen
        sessionTitle={session.title}
        authenticating={authenticating}
        onUnlock={() => {
          void revealLockedNote(tab.id);
        }}
      />
    ) : null;

  return (
    <UnlockedTabContentNote
      tab={tab}
      standaloneWindow={standaloneWindow}
      lockOverlay={lockOverlay}
    />
  );
}

function UnlockedTabContentNote({
  standaloneWindow,
  tab,
  lockOverlay,
}: {
  standaloneWindow: boolean;
  tab: Extract<Tab, { type: "sessions" }>;
  lockOverlay: React.ReactNode;
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
    <>
      {tab.state.autoStart && !standaloneWindow ? (
        // Stay mounted under the lock overlay so a locked session can clear
        // autoStart instead of blocking later scheduled meetings.
        <ScheduledSessionAutoStart sessionId={tab.id} />
      ) : null}
      <SearchProvider>
        <AudioPlayer.Provider sessionId={tab.id} url={audioUrl ?? ""}>
          <TabContentNoteInner
            tab={tab}
            standaloneWindow={standaloneWindow}
            audioUrlReady={Boolean(audioUrl)}
            audioExists={audioExists}
            lockOverlay={lockOverlay}
          />
        </AudioPlayer.Provider>
      </SearchProvider>
    </>
  );
}

function TabContentNoteInner({
  tab,
  standaloneWindow,
  audioUrlReady,
  audioExists,
  lockOverlay,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
  standaloneWindow: boolean;
  audioUrlReady: boolean;
  audioExists: boolean;
  lockOverlay: React.ReactNode;
}) {
  const noteInputRef = React.useRef<NoteInputHandle>(null);

  const sessionId = tab.id;
  const [editingTranscriptSessionId, setEditingTranscriptSessionId] =
    React.useState<string | null>(null);
  const transcriptEditMode = editingTranscriptSessionId === sessionId;
  usePendingUpload(sessionId, !lockOverlay);

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
  const enhancedNoteIds = useEnhancedNotes(sessionId);
  const session = useSession(sessionId);
  const sessionEvent = session ? getSessionEvent(session) : null;
  const contentHydrated = session !== null;
  useEnsureDefaultSummaryFromState({
    batchError: Boolean(batchError),
    enabled: contentHydrated && !lockOverlay,
    enhancedNoteCount: enhancedNoteIds.length,
    hasTranscript,
    memoTemplateId: session?.raw_template_id,
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
      enhancedNoteIds,
      canShowTranscript,
    );
  }, [tab.state.view, isLiveSessionActive, enhancedNoteIds, canShowTranscript]);
  useAutoFocusEditor({
    sessionId,
    noteInputRef,
    enabled: !lockOverlay,
  });

  const showTopAudioPlayer = shouldShowSessionTopAudioPlayer({
    audioExists,
    audioUrlReady,
    currentView,
    sessionMode,
  });

  const handleTabChange = React.useCallback(
    (view: typeof currentView) => {
      if (view.type !== "transcript") {
        blurActiveTranscriptEditor();
        setEditingTranscriptSessionId(null);
      }
      noteInputRef.current?.prepareForTabChange();
      updateSessionTabState(tab, { ...tab.state, view });
    },
    [tab, updateSessionTabState],
  );
  const handleTranscriptEditModeChange = React.useCallback(
    (editMode: boolean) => {
      if (!editMode) {
        blurActiveTranscriptEditor();
      }
      setEditingTranscriptSessionId(editMode ? sessionId : null);
    },
    [sessionId],
  );
  return (
    <>
      <SessionSurface
        overlay={lockOverlay}
        header={
          lockOverlay ? undefined : (
            <OuterHeader
              sessionId={sessionId}
              currentView={currentView}
              tab={tab}
              standaloneWindow={standaloneWindow}
              viewSwitcher={
                <SessionViewSwitcher
                  sessionId={sessionId}
                  editorTabs={editorTabs}
                  currentTab={currentView}
                  handleTabChange={handleTabChange}
                  isTranscribing={isTranscribing}
                  transcriptEditMode={transcriptEditMode}
                  onTranscriptEditModeChange={handleTranscriptEditModeChange}
                />
              }
            />
          )
        }
        floatingButton={
          lockOverlay ? undefined : (
            <FloatingActionButton
              allowListening={!standaloneWindow}
              audioExists={audioExists}
              currentView={currentView}
              skipReason={skipReason}
              tab={tab}
            />
          )
        }
      >
        <div className="flex h-full min-h-0 flex-col">
          {!lockOverlay ? (
            <PendingProposalsBanner sessionId={sessionId} />
          ) : null}
          {showTopAudioPlayer && !lockOverlay ? (
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
            {session ? (
              <NoteInput
                ref={noteInputRef}
                tab={tab}
                rawMd={session.raw_md}
                sessionTitle={session.title}
                eventTitle={sessionEvent?.title}
                eventDescription={sessionEvent?.description}
                editorTabs={editorTabs}
                currentTab={currentView}
                handleTabChange={handleTabChange}
                sessionMode={sessionMode}
                transcriptEditMode={transcriptEditMode}
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

function blurActiveTranscriptEditor() {
  const activeElement = document.activeElement;
  if (
    activeElement instanceof HTMLElement &&
    activeElement.matches("[data-transcript-editor]")
  ) {
    activeElement.blur();
  }
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

function usePendingUpload(sessionId: string, enabled = true) {
  const { processFile } = useUploadFile(sessionId);
  const processFileRef = useRef(processFile);
  processFileRef.current = processFile;

  useEffect(() => {
    if (!enabled) return;
    const pending = consumePendingUpload(sessionId);
    if (pending) {
      processFileRef.current(pending.filePath, pending.kind);
    }
  }, [enabled, sessionId]);
}

function useAutoFocusEditor({
  sessionId,
  noteInputRef,
  enabled = true,
}: {
  sessionId: string;
  noteInputRef: React.RefObject<NoteInputHandle | null>;
  enabled?: boolean;
}) {
  const autoFocusedSessionRef = useRef<string | null>(null);
  const sessionReady = useSession(sessionId) != null;

  useEffect(() => {
    if (!enabled || !sessionReady) return;
    if (autoFocusedSessionRef.current === sessionId) return;

    autoFocusedSessionRef.current = sessionId;
    const frame = requestAnimationFrame(() => {
      noteInputRef.current?.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [enabled, noteInputRef, sessionId, sessionReady]);
}
