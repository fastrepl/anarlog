import { useEffect, useRef } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";
import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";

import { PersistentChatPanel } from "~/chat/components/persistent-chat";

const CHAT_MIN_WIDTH_PX = 280;

export function MainChatPanels({
  autoSaveId,
  isRightPanelOpen,
  rightPanelMode,
  children,
}: {
  autoSaveId: string;
  isRightPanelOpen: boolean;
  rightPanelMode: string;
  children: React.ReactNode;
}) {
  const previousModeRef = useRef(rightPanelMode);
  const bodyPanelRef = useRef<ImperativePanelHandle>(null);
  const chatPanelContainerRef = useRef<HTMLDivElement>(null);
  const bodyPanelContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const isOpeningRightPanel =
      rightPanelMode === "RightPanelOpen" &&
      previousModeRef.current !== "RightPanelOpen";
    const isClosingRightPanel =
      rightPanelMode !== "RightPanelOpen" &&
      previousModeRef.current === "RightPanelOpen";

    if (isOpeningRightPanel) {
      if (bodyPanelRef.current) {
        const currentSize = bodyPanelRef.current.getSize();
        bodyPanelRef.current.resize(currentSize);
      }
      windowsCommands
        .windowExpandWidth(400, null, true, false)
        .catch(console.error);
    } else if (isClosingRightPanel) {
      windowsCommands.windowRestoreWidth().catch(console.error);
    }

    previousModeRef.current = rightPanelMode;
  }, [rightPanelMode]);

  return (
    <>
      <ResizablePanelGroup
        direction="horizontal"
        className="flex min-h-0 flex-1 overflow-hidden"
        autoSaveId={autoSaveId}
      >
        <ResizablePanel
          ref={bodyPanelRef}
          className="min-h-0 flex-1 overflow-hidden"
        >
          <div ref={bodyPanelContainerRef} className="h-full min-h-0">
            {children}
          </div>
        </ResizablePanel>
        {isRightPanelOpen && (
          <>
            <ResizableHandle className="w-0" />
            <ResizablePanel
              defaultSize={30}
              minSize={20}
              maxSize={50}
              className="min-h-0 overflow-hidden"
              style={{ minWidth: CHAT_MIN_WIDTH_PX }}
            >
              <div
                ref={chatPanelContainerRef}
                className="mx-2 -mb-1 h-[calc(100%+0.25rem)] min-h-0 overflow-hidden"
              />
            </ResizablePanel>
          </>
        )}
      </ResizablePanelGroup>

      <PersistentChatPanel
        panelContainerRef={chatPanelContainerRef}
        floatingContainerRef={bodyPanelContainerRef}
      />
    </>
  );
}
