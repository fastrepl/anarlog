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

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    windowExpandWidth: mocks.windowExpandWidth,
    windowRestoreWidth: mocks.windowRestoreWidth,
  },
}));

vi.mock("@anlg/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    autoSaveId,
    children,
    "data-main-chat-panel-group": mainChatPanelGroup,
    direction,
  }: {
    autoSaveId?: string;
    children: React.ReactNode;
    "data-main-chat-panel-group"?: boolean;
    direction: string;
  }) => (
    <div
      data-auto-save-id={autoSaveId}
      data-direction={direction}
      data-main-chat-panel-group={mainChatPanelGroup}
      data-testid="panel-group"
    >
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

import { ZOOM_STORAGE_KEY } from "~/shared/zoom";

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
    window.localStorage.removeItem(ZOOM_STORAGE_KEY);
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
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
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
  });

  it("keeps Automations chat docked without a floating chat host", () => {
    mocks.chatMode = "FloatingOpen";
    mocks.currentTab = { type: "automations" };

    render(
      <MainChatPanels>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    expect(screen.getAllByTestId("panel")).toHaveLength(2);
    expect(screen.getByTestId("chat-view").dataset.layout).toBe("right-panel");
    expect(screen.queryByTestId("persistent-chat-panel")).toBeNull();
    expect(mocks.persistentChatPanel).not.toHaveBeenCalled();
  });

  it.each([
    ["automations", "automations", {}, "800"],
    ["settings", "settings", {}, "min(900px, 100%)"],
    ["sessions", "sessions", {}, "700"],
    ["empty", "empty", {}, "700"],
    [
      "standalone sessions",
      "sessions",
      { leftSidebarAvailable: false, noteSurfaceMinWidth: 420 },
      "420",
    ],
  ] as const)(
    "reserves the main-body min width for the %s surface",
    (_label, tabType, props, expectedMinWidth) => {
      mocks.currentTab = { type: tabType };

      render(renderPanels(null, props, <div data-testid="main-content" />));

      expect(screen.getAllByTestId("panel")[0]?.dataset.minWidth).toBe(
        expectedMinWidth,
      );
    },
  );

  it.each([
    {
      name: "a standalone note for docked chat",
      chatMode: "RightPanelOpen",
      leftSidebarExpanded: true,
      widths: {
        bodyPanelWidth: 420,
        leftSidebarWidth: 200,
        panelGroupWidth: 720,
        rightPanelWidth: 320,
      },
      props: { leftSidebarAvailable: false, noteSurfaceMinWidth: 420 },
      sidebarAttr: "data-left-sidebar-chrome",
      tabType: "sessions",
      expectedArgs: [20, null, false, false, true],
    },
    {
      name: "the left sidebar",
      chatMode: "FloatingClosed",
      leftSidebarExpanded: true,
      widths: { bodyPanelWidth: 640, leftSidebarWidth: 200 },
      sidebarAttr: "data-left-sidebar-chrome",
      tabType: "sessions",
      expectedArgs: [60, null, false, true, false],
    },
    {
      name: "the left sidebar at a zoomed scale",
      chatMode: "FloatingClosed",
      leftSidebarExpanded: true,
      zoom: "3",
      widths: { bodyPanelWidth: 640, leftSidebarWidth: 200 },
      sidebarAttr: "data-left-sidebar-chrome",
      tabType: "sessions",
      expectedArgs: [180, null, false, true, false],
    },
    {
      name: "the expanded sidebar panel without sidebar chrome",
      chatMode: "FloatingClosed",
      leftSidebarExpanded: true,
      widths: { bodyPanelWidth: 720, leftSidebarWidth: 276 },
      sidebarAttr: "data-left-sidebar-panel-content",
      tabType: "sessions",
      expectedArgs: [56, null, false, true, false],
    },
    {
      name: "right for docked chat",
      chatMode: "RightPanelOpen",
      leftSidebarExpanded: false,
      widths: {
        bodyPanelWidth: 460,
        leftSidebarWidth: 0,
        rightPanelWidth: 120,
      },
      sidebarAttr: null,
      tabType: "sessions",
      expectedArgs: [240, null, false, false, true],
    },
    {
      name: "right when docked chat is narrower than 320px",
      chatMode: "RightPanelOpen",
      leftSidebarExpanded: true,
      widths: {
        bodyPanelWidth: 700,
        leftSidebarWidth: 200,
        rightPanelWidth: 120,
      },
      sidebarAttr: "data-left-sidebar-chrome",
      tabType: "sessions",
      expectedArgs: [200, null, false, false, true],
    },
  ] as const)(
    "expands the window for $name",
    ({
      chatMode,
      leftSidebarExpanded,
      zoom,
      widths,
      props,
      sidebarAttr,
      tabType,
      expectedArgs,
    }) => {
      mocks.chatMode = chatMode;
      mocks.currentTab = { type: tabType };
      mocks.leftSidebarExpanded = leftSidebarExpanded;
      if (zoom) {
        window.localStorage.setItem(ZOOM_STORAGE_KEY, zoom);
      }
      mockPanelWidths(widths);

      render(renderPanels(sidebarAttr, props));

      expect(mocks.windowExpandWidth).toHaveBeenCalledWith(...expectedArgs);
      expect(mocks.setLeftSidebarExpanded).not.toHaveBeenCalled();
    },
  );

  it("collapses the left sidebar when docked chat would make the note surface narrower than 500px", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 650,
      leftSidebarWidth: 200,
      rightPanelWidth: 320,
    });

    render(renderPanels("data-left-sidebar-chrome"));

    expect(mocks.setLeftSidebarExpanded).toHaveBeenCalledWith(false);
    expect(mocks.windowExpandWidth).not.toHaveBeenCalled();
  });

  it("does not restore window width after closing a left-sidebar expansion", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 640,
      leftSidebarWidth: 200,
    });

    const { rerender } = render(renderPanels("data-left-sidebar-chrome"));

    expect(mocks.windowExpandWidth).toHaveBeenCalledWith(
      60,
      null,
      false,
      true,
      false,
    );

    mocks.leftSidebarExpanded = false;
    rerender(renderPanels("data-left-sidebar-chrome"));

    expect(mocks.windowRestoreWidth).not.toHaveBeenCalled();
  });

  it.each([
    [
      false,
      {
        bodyPanelWidth: 460,
        leftSidebarWidth: 0,
        rightPanelWidth: 320,
      },
      null,
    ],
    [
      true,
      {
        bodyPanelWidth: 700,
        leftSidebarWidth: 200,
        rightPanelWidth: 120,
      },
      "data-left-sidebar-chrome",
    ],
  ] as const)(
    "restores window width after docked chat closes with the sidebar expanded=%s",
    (leftSidebarExpanded, widths, sidebarAttr) => {
      mocks.chatMode = "RightPanelOpen";
      mocks.currentTab = { type: "sessions" };
      mocks.leftSidebarExpanded = leftSidebarExpanded;
      mockPanelWidths(widths);

      const panels = renderPanels(sidebarAttr);
      const { rerender } = render(panels);

      expect(mocks.windowExpandWidth).toHaveBeenCalled();
      mocks.windowRestoreWidth.mockClear();

      mocks.chatMode = "FloatingClosed";
      rerender(renderPanels(sidebarAttr));

      expect(mocks.windowRestoreWidth).toHaveBeenCalledTimes(1);
    },
  );

  it("does not restore window width when leaving a meeting for settings with chat still open", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    mockPanelWidths({
      bodyPanelWidth: 700,
      leftSidebarWidth: 200,
      rightPanelWidth: 120,
    });

    const { rerender } = render(renderPanels("data-left-sidebar-chrome"));

    expect(mocks.windowExpandWidth).toHaveBeenCalled();
    mocks.windowRestoreWidth.mockClear();

    mocks.currentTab = { type: "settings" };
    rerender(renderPanels("data-left-sidebar-chrome"));

    expect(mocks.windowRestoreWidth).not.toHaveBeenCalled();
  });

  it("collapses the left sidebar when a window resize would make the note surface narrower than 500px", () => {
    mocks.currentTab = { type: "sessions" };
    mocks.leftSidebarExpanded = true;
    const panelWidths = {
      bodyPanelWidth: 720,
      leftSidebarWidth: 200,
    };
    mockPanelWidths(panelWidths);

    render(renderPanels("data-left-sidebar-chrome"));

    expect(mocks.setLeftSidebarExpanded).not.toHaveBeenCalled();

    panelWidths.bodyPanelWidth = 690;
    fireEvent.resize(window);

    expect(mocks.setLeftSidebarExpanded).toHaveBeenCalledWith(false);
  });
});

function renderPanels(
  sidebarAttr:
    | "data-left-sidebar-chrome"
    | "data-left-sidebar-panel-content"
    | null,
  props: {
    leftSidebarAvailable?: boolean;
    noteSurfaceMinWidth?: number;
  } = {},
  children: React.ReactNode = (
    <div data-chat-floating-anchor>
      <div data-session-surface />
    </div>
  ),
) {
  return (
    <MainChatPanels {...props}>
      {sidebarAttr ? <div {...{ [sidebarAttr]: "" }} /> : null}
      {children}
    </MainChatPanels>
  );
}

function mockPanelWidths(widths: {
  bodyPanelWidth: number;
  leftSidebarWidth: number;
  panelGroupWidth?: number;
  rightPanelWidth?: number;
}) {
  restorePanelWidths?.();
  const spy = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function getBoundingClientRectMock(this: HTMLElement) {
      if (this.hasAttribute("data-main-body-panel-container")) {
        return rectWithWidth(widths.bodyPanelWidth);
      }

      if (this.hasAttribute("data-main-chat-panel-group")) {
        return rectWithWidth(widths.panelGroupWidth ?? 0);
      }

      if (
        this.hasAttribute("data-left-sidebar-chrome") ||
        this.hasAttribute("data-left-sidebar-panel-content")
      ) {
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
