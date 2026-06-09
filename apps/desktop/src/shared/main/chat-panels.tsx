import { useRef } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";

import { ChatPanelFrame, ChatSessionHost } from "~/chat/components/chat-panel";
import { PersistentChatPanel } from "~/chat/components/persistent-chat";
import { useShell } from "~/contexts/shell";

const RIGHT_CHAT_PANEL_MIN_WIDTH_PX = 320;

export function MainChatPanels({ children }: { children: React.ReactNode }) {
  const { chat } = useShell();
  const bodyPanelContainerRef = useRef<HTMLDivElement>(null);
  const isRightPanelOpen = chat.mode === "RightPanelOpen";

  return (
    <ChatSessionHost>
      {(sessionProps) => (
        <>
          <ResizablePanelGroup
            autoSaveId="main-chat"
            direction="horizontal"
            className="flex min-h-0 flex-1 overflow-hidden"
          >
            <ResizablePanel className="min-h-0 flex-1 overflow-hidden">
              <div
                ref={bodyPanelContainerRef}
                className="h-full min-h-0 min-w-0 flex-1 overflow-hidden"
              >
                {children}
              </div>
            </ResizablePanel>
            {isRightPanelOpen ? (
              <>
                <ResizableHandle className="w-0" />
                <ResizablePanel
                  defaultSize={30}
                  minSize={20}
                  maxSize={50}
                  className="min-h-0 overflow-hidden"
                  style={{ minWidth: RIGHT_CHAT_PANEL_MIN_WIDTH_PX }}
                >
                  <div
                    data-chat-right-panel
                    className="border-border bg-card -mb-1 h-[calc(100%+0.25rem)] min-h-0 overflow-hidden rounded-tr-xl border-x"
                  >
                    <ChatPanelFrame
                      layout="right-panel"
                      onOpenFloating={() => chat.sendEvent({ type: "OPEN" })}
                      sessionProps={sessionProps}
                    />
                  </div>
                </ResizablePanel>
              </>
            ) : null}
          </ResizablePanelGroup>

          <PersistentChatPanel
            floatingContainerRef={bodyPanelContainerRef}
            sessionProps={sessionProps}
          />
        </>
      )}
    </ChatSessionHost>
  );
}
