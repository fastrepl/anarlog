import * as stylex from "@stylexjs/stylex";
import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useLayoutEffect, useRef } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as windowsCommands } from "@anlg/plugin-windows";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@anlg/ui/components/ui/resizable";

import {
  AUTOMATIONS_SURFACE_MIN_WIDTH_PX,
  NOTE_SURFACE_MIN_WIDTH_PX,
  usesNoteSurfaceMinWidth,
} from "./layout-widths";

import { ChatPanelFrame, ChatSessionHost } from "~/chat/components/chat-panel";
import { PersistentChatPanel } from "~/chat/components/persistent-chat";
import { useShell } from "~/contexts/shell";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const RIGHT_CHAT_PANEL_MIN_WIDTH_PX = 320;
const LEFT_SIDEBAR_MIN_WIDTH_PX = 200;

export function MainChatPanels({
  autoSaveId = "main-chat",
  children,
  leftSidebarAvailable = true,
  noteSurfaceMinWidth = NOTE_SURFACE_MIN_WIDTH_PX,
}: {
  autoSaveId?: string;
  children: React.ReactNode;
  leftSidebarAvailable?: boolean;
  noteSurfaceMinWidth?: number;
}) {
  const { chat, leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const bodyPanelContainerRef = useRef<HTMLDivElement>(null);
  const isAutomationsTab = currentTab?.type === "automations";
  const isRightPanelOpen = isAutomationsTab || chat.mode === "RightPanelOpen";
  const leftSidebarExpanded = leftSidebarAvailable && leftsidebar.expanded;
  const reserveNoteSurfaceMinWidth = usesNoteSurfaceMinWidth(currentTab);
  const collapseLeftSidebar = useCallback(() => {
    leftsidebar.setExpanded(false);
  }, [leftsidebar.setExpanded]);
  const bodyMinWidth = getMainBodyMinWidth({
    currentTab,
    leftSidebarExpanded,
    noteSurfaceMinWidth,
  });

  useNoteSurfaceWindowWidthGuard({
    bodyPanelContainerRef,
    enabled: reserveNoteSurfaceMinWidth,
    leftPanelOpen: leftSidebarExpanded,
    collapseLeftPanel: collapseLeftSidebar,
    noteSurfaceMinWidth,
    rightPanelOpen: isRightPanelOpen,
  });

  return (
    <ChatSessionHost>
      {(sessionProps) => (
        <>
          <ResizablePanelGroup
            autoSaveId={autoSaveId}
            data-main-chat-panel-group
            direction="horizontal"
            sx={styles.panelGroup}
          >
            <ResizablePanel
              sx={styles.bodyPanel}
              style={{ minWidth: bodyMinWidth }}
            >
              <div
                ref={bodyPanelContainerRef}
                data-main-body-panel-container
                {...stylex.props(styles.body)}
              >
                {children}
              </div>
            </ResizablePanel>
            {isRightPanelOpen ? (
              <>
                <ResizableHandle sx={styles.resizeHandle} />
                <ResizablePanel
                  defaultSize={30}
                  minSize={20}
                  maxSize={50}
                  sx={styles.rightPanel}
                  style={{ minWidth: RIGHT_CHAT_PANEL_MIN_WIDTH_PX }}
                >
                  <div
                    data-chat-right-panel
                    {...stylex.props(styles.rightPanelContent)}
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

          {isAutomationsTab ? null : (
            <PersistentChatPanel
              floatingContainerRef={bodyPanelContainerRef}
              sessionProps={sessionProps}
            />
          )}
        </>
      )}
    </ChatSessionHost>
  );
}

const styles = stylex.create({
  body: {
    flex: "1",
    height: "100%",
    minHeight: 0,
    minWidth: 0,
    overflow: "hidden",
  },
  bodyPanel: {
    flex: "1",
    minHeight: 0,
    overflow: "hidden",
  },
  panelGroup: {
    display: "flex",
    flex: "1",
    minHeight: 0,
    overflow: "hidden",
  },
  resizeHandle: {
    width: 0,
  },
  rightPanel: {
    minHeight: 0,
    overflow: "hidden",
  },
  rightPanelContent: {
    backgroundColor: colors.card,
    borderLeftColor: colors.border,
    borderLeftStyle: "solid",
    borderLeftWidth: "1px",
    borderRightColor: colors.border,
    borderRightStyle: "solid",
    borderRightWidth: "1px",
    borderTopRightRadius: radii.xl,
    height: "calc(100% + 0.25rem)",
    marginBottom: "-0.25rem",
    minHeight: 0,
    overflow: "hidden",
  },
});

export { styles as chatPanelStyles };

function getMainBodyMinWidth({
  currentTab,
  leftSidebarExpanded,
  noteSurfaceMinWidth,
}: {
  currentTab: Tab | null;
  leftSidebarExpanded: boolean;
  noteSurfaceMinWidth: number;
}) {
  if (currentTab?.type === "automations") {
    return (
      AUTOMATIONS_SURFACE_MIN_WIDTH_PX +
      (leftSidebarExpanded ? LEFT_SIDEBAR_MIN_WIDTH_PX : 0)
    );
  }

  if (!usesNoteSurfaceMinWidth(currentTab)) {
    return undefined;
  }

  return (
    noteSurfaceMinWidth + (leftSidebarExpanded ? LEFT_SIDEBAR_MIN_WIDTH_PX : 0)
  );
}

function useNoteSurfaceWindowWidthGuard({
  bodyPanelContainerRef,
  collapseLeftPanel,
  enabled,
  leftPanelOpen,
  noteSurfaceMinWidth,
  rightPanelOpen,
}: {
  bodyPanelContainerRef: React.RefObject<HTMLDivElement | null>;
  collapseLeftPanel: () => void;
  enabled: boolean;
  leftPanelOpen: boolean;
  noteSurfaceMinWidth: number;
  rightPanelOpen: boolean;
}) {
  const restorableExpansionCountRef = useRef(0);
  const lastVisibleBodyWidthRef = useRef<number | null>(null);
  const previousStateRef = useRef({
    enabled: false,
    leftPanelOpen: false,
    rightPanelOpen: false,
  });

  const restoreWidthExpansions = useCallback(() => {
    const restoreCount = restorableExpansionCountRef.current;
    restorableExpansionCountRef.current = 0;

    for (let i = 0; i < restoreCount; i += 1) {
      void windowsCommands.windowRestoreWidth();
    }
  }, []);

  useLayoutEffect(() => {
    const previousState = previousStateRef.current;
    const hasOpenPanel = enabled && (leftPanelOpen || rightPanelOpen);
    const rightPanelJustClosed =
      previousState.rightPanelOpen && !rightPanelOpen;

    if (
      rightPanelJustClosed ||
      (enabled && !leftPanelOpen && !rightPanelOpen)
    ) {
      restoreWidthExpansions();
    }

    if (!hasOpenPanel) {
      previousStateRef.current = { enabled, leftPanelOpen, rightPanelOpen };
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
      bodyWidth - leftSidebarWidth < noteSurfaceMinWidth;

    if (shouldCollapseLeftPanelForRightPanel) {
      collapseLeftPanel();
      leftSidebarWidth = 0;
    }

    if (!isTauri()) {
      return;
    }

    const requiredBodyWidth = noteSurfaceMinWidth + leftSidebarWidth;
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
    const restoreOnClose = !expandLeft;

    if (restoreOnClose) {
      restorableExpansionCountRef.current += 1;
    }

    void windowsCommands.windowExpandWidth(
      widthDeficit,
      null,
      false,
      expandLeft,
      restoreOnClose,
    );
  }, [
    bodyPanelContainerRef,
    collapseLeftPanel,
    enabled,
    leftPanelOpen,
    noteSurfaceMinWidth,
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
        noteSurfaceMinWidth,
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
    noteSurfaceMinWidth,
    rightPanelOpen,
  ]);

  useLayoutEffect(() => restoreWidthExpansions, [restoreWidthExpansions]);
}

function collapseLeftPanelIfNoteSurfaceWouldShrink({
  bodyPanel,
  collapseLeftPanel,
  lastVisibleBodyWidthRef,
  noteSurfaceMinWidth,
}: {
  bodyPanel: HTMLElement;
  collapseLeftPanel: () => void;
  lastVisibleBodyWidthRef: React.MutableRefObject<number | null>;
  noteSurfaceMinWidth: number;
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

  if (noteSurfaceWidth < noteSurfaceMinWidth) {
    collapseLeftPanel();
  }
}

function getVisibleBodyWidth(bodyPanel: HTMLElement) {
  const bodyWidth = bodyPanel.getBoundingClientRect().width;
  const widthContainer =
    bodyPanel.closest<HTMLElement>("[data-main-chat-panel-group]") ??
    bodyPanel.closest<HTMLElement>("[data-testid='main-app-shell']");
  if (!widthContainer) {
    return bodyWidth;
  }

  const containerWidth = widthContainer.getBoundingClientRect().width;
  if (containerWidth <= 0) {
    return bodyWidth;
  }

  const rightPanel = widthContainer.querySelector<HTMLElement>(
    "[data-chat-right-panel]",
  );
  const rightPanelWidth = rightPanel?.getBoundingClientRect().width ?? 0;
  const visibleContainerBodyWidth = Math.max(
    0,
    containerWidth - rightPanelWidth,
  );

  if (bodyWidth <= 0) {
    return visibleContainerBodyWidth;
  }

  return Math.min(bodyWidth, visibleContainerBodyWidth);
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
