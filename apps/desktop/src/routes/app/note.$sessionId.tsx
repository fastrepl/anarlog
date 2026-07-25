import { createFileRoute } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useLayoutEffect, useMemo } from "react";

import { useShell } from "~/contexts/shell";
import { ClassicMainLayout } from "~/main/layout";
import { TabContentNote } from "~/session";
import { MainChatPanels } from "~/shared/main";
import {
  getEscapeShortcutContext,
  shouldSkipEscapeShortcut,
} from "~/shared/useMainShortcuts";
import { StandaloneWindowShell } from "~/shared/window-shell";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

const STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX = 420;

export const Route = createFileRoute("/app/note/$sessionId")({
  component: StandaloneNoteWindow,
});

export function StandaloneNoteWindow() {
  const { sessionId } = Route.useParams();

  return (
    <ClassicMainLayout includeServices={false}>
      <StandaloneNoteContent sessionId={sessionId} />
    </ClassicMainLayout>
  );
}

function StandaloneNoteContent({ sessionId }: { sessionId: string }) {
  useCloseStandaloneNoteWindowOnEscape();
  useAttachStandaloneNoteToLiveSession(sessionId);
  const tab = useStandaloneNoteTab(sessionId);

  return (
    <StandaloneWindowShell topDragRegion={false}>
      <div className="bg-background flex h-screen w-screen">
        <MainChatPanels
          autoSaveId="standalone-note-chat"
          leftSidebarAvailable={false}
          noteSurfaceMinWidth={STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX}
        >
          <TabContentNote tab={tab} standaloneWindow />
        </MainChatPanels>
      </div>
    </StandaloneWindowShell>
  );
}

export function useAttachStandaloneNoteToLiveSession(sessionId: string) {
  const attachLiveSession = useListener((state) => state.attachLiveSession);
  const hasLiveSessionEvents = useListener((state) =>
    Boolean(state.live.eventUnlistenersBySession[sessionId]),
  );

  useEffect(() => {
    if (hasLiveSessionEvents) {
      return;
    }

    void attachLiveSession(sessionId);
  }, [attachLiveSession, hasLiveSessionEvents, sessionId]);
}

export function useStandaloneNoteTab(sessionId: string) {
  const tab = useMemo(
    () =>
      ({
        active: true,
        id: sessionId,
        pinned: false,
        slotId: `note-window-${sessionId}`,
        state: { view: null, autoStart: null },
        type: "sessions",
      }) satisfies Extract<Tab, { type: "sessions" }>,
    [sessionId],
  );

  const storeTab = useTabs((state) =>
    state.tabs.find(
      (candidate): candidate is Extract<Tab, { type: "sessions" }> =>
        candidate.type === "sessions" && candidate.id === sessionId,
    ),
  );

  useLayoutEffect(() => {
    const state = useTabs.getState();
    const existingTab = state.tabs.find(
      (candidate): candidate is Extract<Tab, { type: "sessions" }> =>
        candidate.type === "sessions" && candidate.id === sessionId,
    );

    if (existingTab) {
      state.select(existingTab);
      return;
    }

    state.openCurrent(tab);
  }, [sessionId, tab]);

  return storeTab ?? tab;
}

export function useCloseStandaloneNoteWindowOnEscape() {
  const { chat } = useShell();

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      const escapeContext = getEscapeShortcutContext(event.target);
      window.setTimeout(() => {
        if (shouldSkipEscapeShortcut(event, escapeContext)) {
          return;
        }

        event.preventDefault();
        if (chat.mode !== "FloatingClosed") {
          chat.sendEvent({ type: "CLOSE" });
          return;
        }

        void getCurrentWindow().close();
      });
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  }, [chat.mode, chat.sendEvent]);
}
