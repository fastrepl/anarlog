import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useLayoutEffect, useRef } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";

import { NOTE_SURFACE_MIN_WIDTH_PX } from "./layout-widths";

import { ChatPanelFrame, ChatSessionHost } from "~/chat/components/chat-panel";
import { PersistentChatPanel } from "~/chat/components/persistent-chat";
import { useShell } from "~/contexts/shell";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const RIGHT_CHAT_PANEL_MIN_WIDTH_PX = 320;
const LEFT_SIDEBAR_MIN_WIDTH_PX = 200;

export function MainChatPanels({ children }: { children: React.ReactNode }) {
  const { chat, leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const bodyPanelContainerRef = useRef<HTMLDivElement>(null);
  const isRightPanelOpen = chat.mode === "RightPanelOpen";
  const isSessionTab = currentTab?.type === "sessions";
  const collapseLeftSidebar = useCallback(() => {
    leftsidebar.setExpanded(false);
  }, [leftsidebar.setExpanded]);
  const bodyMinWidth = getMainBodyMinWidth({
    currentTab,
    leftSidebarExpanded: leftsidebar.expanded,
  });

  useNoteSurfaceWindowWidthGuard({
    bodyPanelContainerRef,
    enabled: isSessionTab,
    leftPanelOpen: leftsidebar.expanded,
    collapseLeftPanel: collapseLeftSidebar,
    rightPanelOpen: isRightPanelOpen,
  });

  return (
    <ChatSessionHost>
      {(sessionProps) => (
        <>
          <ResizablePanelGroup
            autoSaveId="main-chat"
            direction="horizontal"
            className="flex min-h-0 flex-1 overflow-hidden"
          >
            <ResizablePanel
              className="min-h-0 flex-1 overflow-hidden"
              style={{ minWidth: bodyMinWidth }}
            >
              <div
                ref={bodyPanelContainerRef}
                data-main-body-panel-container
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

function getMainBodyMinWidth({
  currentTab,
  leftSidebarExpanded,
}: {
  currentTab: Tab | null;
  leftSidebarExpanded: boolean;
}) {
  if (currentTab?.type !== "sessions") {
    return undefined;
  }

  return (
    NOTE_SURFACE_MIN_WIDTH_PX +
    (leftSidebarExpanded ? LEFT_SIDEBAR_MIN_WIDTH_PX : 0)
  );
}

function useNoteSurfaceWindowWidthGuard({
  bodyPanelContainerRef,
  collapseLeftPanel,
  enabled,
  leftPanelOpen,
  rightPanelOpen,
}: {
  bodyPanelContainerRef: React.RefObject<HTMLDivElement | null>;
  collapseLeftPanel: () => void;
  enabled: boolean;
  leftPanelOpen: boolean;
  rightPanelOpen: boolean;
}) {
  const expansionCountRef = useRef(0);
  const lastVisibleBodyWidthRef = useRef<number | null>(null);
  const previousStateRef = useRef({
    enabled: false,
    leftPanelOpen: false,
    rightPanelOpen: false,
  });

  const restoreWidthExpansions = useCallback(() => {
    const restoreCount = expansionCountRef.current;
    expansionCountRef.current = 0;

    for (let i = 0; i < restoreCount; i += 1) {
      void windowsCommands.windowRestoreWidth();
    }
  }, []);

  useLayoutEffect(() => {
    const previousState = previousStateRef.current;
    const hasOpenPanel = enabled && (leftPanelOpen || rightPanelOpen);

    if (!hasOpenPanel) {
      previousStateRef.current = { enabled, leftPanelOpen, rightPanelOpen };
      restoreWidthExpansions();
      return;
    }

    const leftPanelJustOpened =
      leftPanelOpen && (!previousState.enabled || !previousState.leftPanelOpen);
    const rightPanelJustOpened =
      rightPanelOpen &&
      (!previousState.enabled || !previousState.rightPanelOpen);

    previousStateRef.current = { enabled, leftPanelOpen, rightPanelOpen };

    if (!leftPanelJustOpened && !rightPanelJustOpened) {
      return;
    }

    const bodyPanel = bodyPanelContainerRef.current;
    if (!bodyPanel) {
      return;
    }

    const bodyWidth = getVisibleBodyWidth(bodyPanel);
    if (bodyWidth <= 0) {
      return;
    }

    let leftSidebarWidth = getLeftSidebarWidth(bodyPanel, leftPanelOpen);
    const rightPanelWidth = getRightPanelWidth(bodyPanel, rightPanelOpen);
    const shouldCollapseLeftPanelForRightPanel =
      rightPanelJustOpened &&
      leftPanelOpen &&
      leftSidebarWidth > 0 &&
      bodyWidth - leftSidebarWidth < NOTE_SURFACE_MIN_WIDTH_PX;

    if (shouldCollapseLeftPanelForRightPanel) {
      collapseLeftPanel();
      leftSidebarWidth = 0;
    }

    if (!isTauri()) {
      return;
    }

    const requiredBodyWidth = NOTE_SURFACE_MIN_WIDTH_PX + leftSidebarWidth;
    const requiredTotalWidth =
      requiredBodyWidth + (rightPanelOpen ? RIGHT_CHAT_PANEL_MIN_WIDTH_PX : 0);
    const visibleTotalWidth = bodyWidth + rightPanelWidth;
    const widthDeficit = Math.ceil(
      Math.max(
        requiredBodyWidth - bodyWidth,
        requiredTotalWidth - visibleTotalWidth,
        rightPanelOpen ? RIGHT_CHAT_PANEL_MIN_WIDTH_PX - rightPanelWidth : 0,
      ),
    );

    if (widthDeficit <= 0) {
      return;
    }

    const expandLeft = leftPanelJustOpened && !rightPanelJustOpened;

    expansionCountRef.current += 1;
    void windowsCommands.windowExpandWidth(
      widthDeficit,
      null,
      false,
      expandLeft,
    );
  }, [
    bodyPanelContainerRef,
    collapseLeftPanel,
    enabled,
    leftPanelOpen,
    restoreWidthExpansions,
    rightPanelOpen,
  ]);

  useLayoutEffect(() => {
    lastVisibleBodyWidthRef.current = null;

    if (!enabled || !leftPanelOpen) {
      return;
    }

    const bodyPanel = bodyPanelContainerRef.current;
    if (!bodyPanel) {
      return;
    }

    const handleResize = () => {
      collapseLeftPanelIfNoteSurfaceWouldShrink({
        bodyPanel,
        collapseLeftPanel,
        lastVisibleBodyWidthRef,
      });
    };

    handleResize();
    window.addEventListener("resize", handleResize);

    const resizeObserver =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(handleResize)
        : null;
    resizeObserver?.observe(bodyPanel);

    const shell = bodyPanel.closest<HTMLElement>(
      "[data-testid='main-app-shell']",
    );
    if (shell) {
      resizeObserver?.observe(shell);
    }

    return () => {
      window.removeEventListener("resize", handleResize);
      resizeObserver?.disconnect();
    };
  }, [
    bodyPanelContainerRef,
    collapseLeftPanel,
    enabled,
    leftPanelOpen,
    rightPanelOpen,
  ]);

  useLayoutEffect(() => restoreWidthExpansions, [restoreWidthExpansions]);
}

function collapseLeftPanelIfNoteSurfaceWouldShrink({
  bodyPanel,
  collapseLeftPanel,
  lastVisibleBodyWidthRef,
}: {
  bodyPanel: HTMLElement;
  collapseLeftPanel: () => void;
  lastVisibleBodyWidthRef: React.MutableRefObject<number | null>;
}) {
  const visibleBodyWidth = getVisibleBodyWidth(bodyPanel);
  if (visibleBodyWidth <= 0) {
    return;
  }

  const lastVisibleBodyWidth = lastVisibleBodyWidthRef.current;
  lastVisibleBodyWidthRef.current = visibleBodyWidth;

  if (
    lastVisibleBodyWidth === null ||
    visibleBodyWidth >= lastVisibleBodyWidth
  ) {
    return;
  }

  const leftSidebarWidth = getLeftSidebarWidth(bodyPanel, true);
  const noteSurfaceWidth = visibleBodyWidth - leftSidebarWidth;

  if (noteSurfaceWidth < NOTE_SURFACE_MIN_WIDTH_PX) {
    collapseLeftPanel();
  }
}

function getVisibleBodyWidth(bodyPanel: HTMLElement) {
  const bodyWidth = bodyPanel.getBoundingClientRect().width;
  const shell = bodyPanel.closest<HTMLElement>(
    "[data-testid='main-app-shell']",
  );
  if (!shell) {
    return bodyWidth;
  }

  const shellWidth = shell.getBoundingClientRect().width;
  if (shellWidth <= 0) {
    return bodyWidth;
  }

  const rightPanel = shell.querySelector<HTMLElement>(
    "[data-chat-right-panel]",
  );
  const rightPanelWidth = rightPanel?.getBoundingClientRect().width ?? 0;
  const visibleShellBodyWidth = Math.max(0, shellWidth - rightPanelWidth);

  if (bodyWidth <= 0) {
    return visibleShellBodyWidth;
  }

  return Math.min(bodyWidth, visibleShellBodyWidth);
}

function getRightPanelWidth(bodyPanel: HTMLElement, rightPanelOpen: boolean) {
  if (!rightPanelOpen) {
    return 0;
  }

  const rightPanel = bodyPanel.ownerDocument.querySelector<HTMLElement>(
    "[data-chat-right-panel]",
  );

  return rightPanel?.getBoundingClientRect().width ?? 0;
}

function getLeftSidebarWidth(bodyPanel: HTMLElement, leftPanelOpen: boolean) {
  if (!leftPanelOpen) {
    return 0;
  }

  const leftSidebarChrome = bodyPanel.querySelector<HTMLElement>(
    "[data-left-sidebar-chrome]",
  );
  const measuredWidth = leftSidebarChrome?.getBoundingClientRect().width ?? 0;

  return measuredWidth > 0 ? measuredWidth : LEFT_SIDEBAR_MIN_WIDTH_PX;
}
