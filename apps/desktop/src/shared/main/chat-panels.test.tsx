import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as
    | "FloatingClosed"
    | "FloatingOpen"
    | "RightPanelOpen",
  currentTab: { type: "empty" } as { type: string } | null,
  leftSidebarExpanded: true,
  persistentChatPanel: vi.fn(),
  sendEvent: vi.fn(),
  sessionProps: { sessionId: "chat-session-1" },
  setLeftSidebarExpanded: vi.fn(),
  windowExpandWidth: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  windowRestoreWidth: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowExpandWidth: mocks.windowExpandWidth,
    windowRestoreWidth: mocks.windowRestoreWidth,
  },
}));

vi.mock("@hypr/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    children,
    direction,
  }: {
    children: React.ReactNode;
    direction: string;
  }) => (
    <div data-direction={direction} data-testid="panel-group">
      {children}
    </div>
  ),
  ResizablePanel: ({
    children,
    className,
    defaultSize,
    maxSize,
    minSize,
    style,
  }: {
    children: React.ReactNode;
    className?: string;
    defaultSize?: number;
    maxSize?: number;
    minSize?: number;
    style?: React.CSSProperties;
  }) => (
    <div
      data-class-name={className}
      data-default-size={defaultSize}
      data-max-size={maxSize}
      data-min-size={minSize}
      data-min-width={style?.minWidth}
      data-testid="panel"
    >
      {children}
    </div>
  ),
  ResizableHandle: ({ className }: { className?: string }) => (
    <div data-class-name={className} data-testid="resize-handle" />
  ),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
    leftsidebar: {
      expanded: mocks.leftSidebarExpanded,
      setExpanded: mocks.setLeftSidebarExpanded,
    },
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("~/chat/components/chat-panel", () => ({
  ChatPanelFrame: ({
    layout,
    onOpenFloating,
    sessionProps,
  }: {
    layout?: "floating" | "right-panel";
    onOpenFloating?: () => void;
    sessionProps: unknown;
  }) => (
    <button
      data-has-session={String(sessionProps === mocks.sessionProps)}
      data-layout={layout}
      data-testid="chat-view"
      type="button"
      onClick={onOpenFloating}
    >
      Chat
    </button>
  ),
  ChatSessionHost: ({
    children,
  }: {
    children: (sessionProps: unknown) => React.ReactNode;
  }) => <>{children(mocks.sessionProps)}</>,
}));

vi.mock("~/chat/components/persistent-chat", () => ({
  PersistentChatPanel: ({
    floatingContainerRef,
    sessionProps,
  }: {
    floatingContainerRef: { current: HTMLDivElement | null };
    sessionProps: unknown;
  }) => {
    mocks.persistentChatPanel(floatingContainerRef, sessionProps);
    return <div data-testid="persistent-chat-panel" />;
  },
}));

import { MainChatPanels } from "./chat-panels";

let restorePanelWidths: (() => void) | null = null;

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
    restorePanelWidths?.();
    restorePanelWidths = null;
    mocks.chatMode = "FloatingClosed";
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mocks.persistentChatPanel.mockClear();
    mocks.sendEvent.mockClear();
    mocks.setLeftSidebarExpanded.mockClear();
    mocks.windowExpandWidth.mockClear();
    mocks.windowRestoreWidth.mockClear();
  });

  it("renders the main content and persistent floating chat host", () => {
    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getByTestId("main-content")).toBeTruthy();
    expect(screen.getByTestId("persistent-chat-panel")).toBeTruthy();
    expect(mocks.persistentChatPanel).toHaveBeenCalledTimes(1);
    expect(mocks.persistentChatPanel.mock.calls[0]?.[0].current).toBeInstanceOf(
      HTMLDivElement,
    );
    expect(mocks.persistentChatPanel.mock.calls[0]?.[1]).toBe(
      mocks.sessionProps,
    );
    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "horizontal",
    );
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("renders the right chat panel when chat is docked", () => {
    mocks.chatMode = "RightPanelOpen";

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getAllByTestId("panel")).toHaveLength(2);
    expect(screen.getByTestId("resize-handle")).toBeTruthy();
    expect(screen.getByTestId("chat-view").dataset.layout).toBe("right-panel");
    expect(screen.getByTestId("chat-view").dataset.hasSession).toBe("true");
    expect(mocks.persistentChatPanel.mock.calls[0]?.[1]).toBe(
      mocks.sessionProps,
    );
    const rightPanel = document.querySelector("[data-chat-right-panel]");

    expect(rightPanel).toBeInstanceOf(HTMLDivElement);
    expect(rightPanel?.className).toContain("bg-card");
    expect(rightPanel?.className).toContain("border-x");
    expect(rightPanel?.className).toContain("border-border");
    expect(rightPanel?.className).not.toContain("border-b-0");
    expect(rightPanel?.className).toContain("rounded-tr-xl");
    expect(rightPanel?.className).not.toContain("rounded-t-xl");
    expect(rightPanel?.className).not.toContain("ml-2");
    expect(rightPanel?.className).not.toContain("mr-1");
  });

  it("reserves enough main-body width for a 500px note surface beside the sidebar", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getAllByTestId("panel")[0]?.dataset.minWidth).toBe("700");
  });

  it("reserves enough main-body width for the empty surface beside the sidebar", () => {
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getAllByTestId("panel")[0]?.dataset.minWidth).toBe("700");
  });

  it("expands left when opening the sidebar would make a note surface narrower than 500px", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 640,
      leftSidebarWidth: 200,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(60, null, false, true);
  });

  it("expands left when opening the sidebar would make the empty surface narrower than 500px", () => {
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 640,
      leftSidebarWidth: 200,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-testid="empty-surface" />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(60, null, false, true);
  });

  it("expands right when docked chat would make a note surface narrower than 500px", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = false;
    mockPanelWidths({
      bodyPanelWidth: 460,
      leftSidebarWidth: 0,
      rightPanelWidth: 120,
    });

    render(
      <MainChatPanels>
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      240,
      null,
      false,
      false,
    );
  });

  it("collapses the left sidebar when docked chat would make the note surface narrower than 500px", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 650,
      leftSidebarWidth: 200,
      rightPanelWidth: 320,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.setLeftSidebarExpanded).toHaveBeenCalledWith(false);
    expect(mocks.windowExpandWidth).not.toHaveBeenCalled();
  });

  it("expands right when docked chat renders narrower than 320px", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 700,
      leftSidebarWidth: 200,
      rightPanelWidth: 120,
    });

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      200,
      null,
      false,
      false,
    );
  });

  it("restores window width expansions after the side panels close", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = false;
    mockPanelWidths({
      bodyPanelWidth: 460,
      leftSidebarWidth: 0,
      rightPanelWidth: 320,
    });

    const renderPanels = () => (
      <MainChatPanels>
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>
    );
    const { rerender } = render(renderPanels());

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      40,
      null,
      false,
      false,
    );

    mocks.chatMode = "FloatingClosed";
    rerender(renderPanels());

    expect(mocks.windowRestoreWidth).toHaveBeenCalledTimes(1);
  });

  it("collapses the left sidebar when a window resize would make the note surface narrower than 500px", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    const panelWidths = {
      bodyPanelWidth: 720,
      leftSidebarWidth: 200,
    };
    mockPanelWidths(panelWidths);

    render(
      <MainChatPanels>
        <div data-left-sidebar-chrome />
        <div data-chat-floating-anchor>
          <div data-session-surface />
        </div>
      </MainChatPanels>,
    );

    expect(mocks.setLeftSidebarExpanded).not.toHaveBeenCalled();

    panelWidths.bodyPanelWidth = 690;
    fireEvent.resize(window);

    expect(mocks.setLeftSidebarExpanded).toHaveBeenCalledWith(false);
  });
});

function mockPanelWidths(widths: {
  bodyPanelWidth: number;
  leftSidebarWidth: number;
  rightPanelWidth?: number;
}) {
  restorePanelWidths?.();
  const spy = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function getBoundingClientRectMock(this: HTMLElement) {
      if (this.hasAttribute("data-main-body-panel-container")) {
        return rectWithWidth(widths.bodyPanelWidth);
      }

      if (this.hasAttribute("data-left-sidebar-chrome")) {
        return rectWithWidth(widths.leftSidebarWidth);
      }

      if (this.hasAttribute("data-chat-right-panel")) {
        return rectWithWidth(widths.rightPanelWidth ?? 0);
      }

      return rectWithWidth(0);
    });
  restorePanelWidths = () => spy.mockRestore();
}

function rectWithWidth(width: number) {
  return {
    bottom: 0,
    height: 0,
    left: 0,
    right: width,
    top: 0,
    width,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  };
}
