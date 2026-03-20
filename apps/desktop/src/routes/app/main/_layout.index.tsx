import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useRef } from "react";

import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";

import { PersistentChatPanel } from "~/chat/components/persistent-chat";
import { useShell } from "~/contexts/shell";
import { useSearch } from "~/search/contexts/ui";
import { Body } from "~/shared/main";
import { LeftSidebar } from "~/sidebar";
import { useTabs } from "~/store/zustand/tabs";
import { commands } from "~/types/tauri.gen";

export const Route = createFileRoute("/app/main/_layout/")({
  component: Component,
});

const CHAT_MIN_WIDTH_PX = 280;

function Component() {
  const { leftsidebar, chat } = useShell();
  const { query } = useSearch();
  const currentTab = useTabs((state) => state.currentTab);
  const isOnboarding = currentTab?.type === "onboarding";
  const previousModeRef = useRef(chat.mode);
  const previousQueryRef = useRef(query);
  const bodyPanelRef = useRef<ImperativePanelHandle>(null);
  const chatPanelContainerRef = useRef<HTMLDivElement>(null);

  const isChatOpen = chat.mode === "RightPanelOpen";

  useEffect(() => {
    if (isOnboarding && leftsidebar.expanded) {
      leftsidebar.setExpanded(false);
    }
  }, [isOnboarding, leftsidebar]);

  useEffect(() => {
    const isOpeningRightPanel =
      chat.mode === "RightPanelOpen" &&
      previousModeRef.current !== "RightPanelOpen";

    if (isOpeningRightPanel && bodyPanelRef.current) {
      const currentSize = bodyPanelRef.current.getSize();
      bodyPanelRef.current.resize(currentSize);
    }

    previousModeRef.current = chat.mode;
  }, [chat.mode]);

  useEffect(() => {
    const isStartingSearch =
      query.trim() !== "" && previousQueryRef.current.trim() === "";

    if (isStartingSearch && !leftsidebar.expanded && !isOnboarding) {
      leftsidebar.setExpanded(true);
      commands.resizeWindowForSidebar().catch(console.error);
    }

    previousQueryRef.current = query;
  }, [query, leftsidebar]);

  return (
    <div
      className="flex h-full gap-1 overflow-hidden bg-stone-50 p-1"
      data-testid="main-app-shell"
    >
      {leftsidebar.expanded && !isOnboarding && <LeftSidebar />}

      <ResizablePanelGroup
        direction="horizontal"
        className="flex flex-1 overflow-hidden"
        autoSaveId="main-chat"
      >
        <ResizablePanel ref={bodyPanelRef} className="flex-1 overflow-hidden">
          <Body />
        </ResizablePanel>
        {isChatOpen && (
          <>
            <ResizableHandle className="w-0" />
            <ResizablePanel
              defaultSize={30}
              minSize={20}
              maxSize={50}
              className="pl-1"
              style={{ minWidth: CHAT_MIN_WIDTH_PX }}
            >
              <div ref={chatPanelContainerRef} className="h-full" />
            </ResizablePanel>
          </>
        )}
      </ResizablePanelGroup>

      <PersistentChatPanel panelContainerRef={chatPanelContainerRef} />
    </div>
  );
}
