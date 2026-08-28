import * as stylex from "@stylexjs/stylex";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { forwardRef, useImperativeHandle, useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  runtimePlatform: null as "windows" | "linux" | null,
  currentTab: {
    active: true,
    pinned: false,
    slotId: "slot-home",
    type: "empty",
  } as ({ type: string } & Record<string, unknown>) | null,
  leftsidebar: {
    expanded: true,
    setExpanded: vi.fn(),
    toggleExpanded: vi.fn(),
  },
  onPanelCollapse: null as null | (() => void),
  onPanelLayout: null as null | ((sizes: number[]) => void),
  onResizeDragging: null as null | ((isDragging: boolean) => void),
  leftSidebarPanelHandle: {
    collapse: vi.fn(),
    expand: vi.fn(),
    getId: vi.fn(() => "classic-main-sidebar-left"),
    getSize: vi.fn(() => 12.5),
    isCollapsed: vi.fn(() => false),
    isExpanded: vi.fn(() => true),
    resize: vi.fn(),
  },
  tabContentRenderCount: 0,
  upcomingMeetingStatus: null as null | {
    itemKey: string;
    label: string;
    title: string;
  },
  setUpcomingMeetingStatus: null as
    | null
    | ((
        status: null | { itemKey: string; label: string; title: string },
      ) => void),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => {
    if (mocks.runtimePlatform === null) {
      throw new Error("Tauri runtime unavailable");
    }

    return mocks.runtimePlatform;
  },
}));

vi.mock("@anlg/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    autoSaveId,
    children,
    dir,
    direction,
    onLayout,
    sx,
  }: {
    autoSaveId?: string;
    children: React.ReactNode;
    dir?: string;
    direction: string;
    onLayout?: (sizes: number[]) => void;
    sx?: stylex.StyleXStyles;
  }) => {
    mocks.onPanelLayout = onLayout ?? null;

    return (
      <div
        data-auto-save-id={autoSaveId}
        data-class-name={stylex.props(sx).className}
        data-dir={dir}
        data-direction={direction}
        data-testid="panel-group"
      >
        {children}
      </div>
    );
  },
  ResizablePanel: forwardRef(
    (
      {
        children,
        collapsedSize,
        collapsible,
        defaultSize,
        id,
        maxSize,
        minSize,
        onCollapse,
        order,
        style,
        sx,
      }: {
        children: React.ReactNode;
        collapsedSize?: number;
        collapsible?: boolean;
        defaultSize?: number;
        id?: string;
        maxSize?: number;
        minSize?: number;
        onCollapse?: () => void;
        order?: number;
        style?: React.CSSProperties;
        sx?: stylex.StyleXStyles;
      },
      ref,
    ) => {
      useImperativeHandle(ref, () => mocks.leftSidebarPanelHandle);

      if (id === "classic-main-sidebar-left") {
        mocks.onPanelCollapse = onCollapse ?? null;
      }

      return (
        <div
          data-class-name={stylex.props(sx).className}
          data-collapsed-size={collapsedSize}
          data-collapsible={collapsible}
          data-default-size={defaultSize}
          data-panel-id={id}
          data-flex-basis={style?.flexBasis}
          data-max-size={maxSize}
          data-min-size={minSize}
          data-flex-grow={style?.flexGrow}
          data-max-width={style?.maxWidth}
          data-min-width={style?.minWidth}
          data-order={order}
          data-transition={style?.transition}
          data-testid="panel"
        >
          {children}
        </div>
      );
    },
  ),
  ResizableHandle: ({
    onDragging,
    sx,
  }: {
    onDragging?: (isDragging: boolean) => void;
    sx?: stylex.StyleXStyles;
  }) => {
    mocks.onResizeDragging = onDragging ?? null;

    return (
      <div
        data-class-name={stylex.props(sx).className}
        data-testid="resize-handle"
      />
    );
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  uniqueIdfromTab: (tab: { type: string }) => tab.type,
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("./shell-sidebar", () => ({
  ClassicMainSidebar: ({
    forceMount = false,
    timelineHeader,
  }: {
    forceMount?: boolean;
    timelineHeader?: React.ReactNode;
  }) =>
    (forceMount || mocks.leftsidebar.expanded) &&
    mocks.currentTab?.type !== "onboarding" ? (
      <aside data-testid="classic-main-sidebar">
        {timelineHeader}
        <div data-sidebar-timeline-scroll />
      </aside>
    ) : null,
}));

vi.mock("./tab-content", () => ({
  ClassicMainTabContent: ({ tab }: { tab: { type: string } }) => {
    mocks.tabContentRenderCount += 1;
    return <div data-tab-type={tab.type} data-testid="tab-content" />;
  },
}));

vi.mock("./sync-status", () => ({
  SyncStatusIndicator: () => <div data-testid="sync-status-indicator" />,
}));

vi.mock("~/sidebar/note-filter-menu", () => ({
  SidebarNoteFilterMenu: () => <button type="button">Filter notes</button>,
}));

vi.mock("./useShortcuts", () => ({
  useClassicMainShortcuts: () => ({ runEscapeShortcut: vi.fn() }),
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({ open: vi.fn() }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
}));

vi.mock("~/sidebar/timeline/upcoming-meeting", () => ({
  useSidebarUpcomingMeetingStatus: () => {
    const [status, setStatus] = useState(mocks.upcomingMeetingStatus);
    mocks.setUpcomingMeetingStatus = setStatus;
    return status;
  },
}));

import { classicMainBodyStyles, ClassicMainBody } from "./body";

function rectWithWidth(width: number) {
  return {
    bottom: 0,
    height: 0,
    left: 0,
    right: width,
    toJSON: () => ({}),
    top: 0,
    width,
    x: 0,
    y: 0,
  } as DOMRect;
}

describe("ClassicMainBody", () => {
  beforeEach(() => {
    cleanup();
    mocks.runtimePlatform = null;
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 1600,
    });
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-home",
      type: "empty",
    };
    mocks.leftsidebar.expanded = true;
    mocks.leftsidebar.setExpanded.mockClear();
    mocks.leftsidebar.toggleExpanded.mockClear();
    mocks.onPanelCollapse = null;
    mocks.onPanelLayout = null;
    mocks.onResizeDragging = null;
    mocks.leftSidebarPanelHandle.collapse.mockClear();
    mocks.leftSidebarPanelHandle.expand.mockClear();
    mocks.leftSidebarPanelHandle.getId.mockClear();
    mocks.leftSidebarPanelHandle.getSize.mockClear();
    mocks.leftSidebarPanelHandle.isCollapsed.mockClear();
    mocks.leftSidebarPanelHandle.isExpanded.mockClear();
    mocks.leftSidebarPanelHandle.resize.mockClear();
    mocks.tabContentRenderCount = 0;
    mocks.upcomingMeetingStatus = null;
    mocks.setUpcomingMeetingStatus = null;
  });

  it("wraps the expanded left sidebar in a persistent resizable panel", () => {
    render(<ClassicMainBody />);

    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "horizontal",
    );
    expect(screen.getByTestId("panel-group").dataset.dir).toBe("ltr");
    expect(screen.getByTestId("panel-group").dataset.autoSaveId).toBe(
      "classic-main-sidebar",
    );
    expect(screen.getByTestId("panel-group").previousElementSibling).toBeNull();
    expect(screen.getByTestId("classic-main-sidebar")).toBeTruthy();
    expectDataStyle(
      screen.getByTestId("resize-handle"),
      classicMainBodyStyles.resizeHandle,
    );
    expectDataStyle(
      screen.getByTestId("resize-handle"),
      classicMainBodyStyles.resizeHandleEnabled,
    );

    const panels = screen.getAllByTestId("panel");
    expect(panels).toHaveLength(2);
    expect(panels[0]?.dataset.panelId).toBe("classic-main-sidebar-left");
    expect(panels[0]?.dataset.order).toBe("1");
    expect(panels[0]?.dataset.collapsible).toBe("true");
    expect(panels[0]?.dataset.collapsedSize).toBe("0");
    expect(panels[0]?.dataset.defaultSize).toBe("12.5");
    expect(panels[0]?.dataset.minSize).toBe("12.5");
    expect(panels[0]?.dataset.maxSize).toBe("22.5");
    expect(panels[0]?.dataset.flexGrow).toBe("var(--left-sidebar-panel-size)");
    expect(panels[0]?.dataset.minWidth).toBe("200");
    expect(panels[0]?.dataset.maxWidth).toBe("360");
    expect(panels[0]?.dataset.transition).toBeUndefined();
    expect(panels[1]?.dataset.panelId).toBe("classic-main-content");
    expect(panels[1]?.dataset.order).toBe("2");

    const sidebarContent = document.querySelector<HTMLElement>(
      "[data-left-sidebar-panel-content]",
    );
    const sidebarChrome = document.querySelector<HTMLElement>(
      "[data-left-sidebar-chrome]",
    );
    const sidebarTimelineHeader = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-header]",
    );

    expectStyle(sidebarContent, classicMainBodyStyles.sidebarContent);
    expectStyle(sidebarContent, classicMainBodyStyles.sidebarContentExpanded);
    expect(sidebarContent?.getAttribute("aria-hidden")).toBe("false");
    expect(sidebarChrome).toBeNull();
    expect(sidebarTimelineHeader).toBeTruthy();
    expectStyle(sidebarTimelineHeader, classicMainBodyStyles.timelineHeader);

    const bodyRoot = screen.getByTestId("panel-group").parentElement;
    expect(bodyRoot?.getAttribute("style")).toContain(
      "--left-sidebar-panel-size: 12.5",
    );

    act(() => {
      mocks.onPanelLayout?.([24, 76]);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "24",
    );
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width")).toBe(
      "24%",
    );
  });

  it("anchors sync status inside the main note area", () => {
    render(<ClassicMainBody showSyncStatus />);

    const mainContentPanel = document.querySelector(
      "[data-main-content-panel]",
    );

    expectStyle(mainContentPanel, classicMainBodyStyles.mainContent);
    expect(screen.getByTestId("sync-status-indicator").parentElement).toBe(
      mainContentPanel,
    );
  });

  it.each([
    ["settings", { state: { tab: "app" } }],
    ["calendar", {}],
    ["contacts", { state: { selected: null } }],
    ["templates", { state: { selectedMineId: null, selectedWebIndex: null } }],
    ["automations", {}],
  ])("keeps the %s left sidebar fixed", (type, extraTabState) => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: `slot-${type}`,
      type,
      ...extraTabState,
    };

    render(<ClassicMainBody />);

    expect(screen.getByTestId("panel-group").dataset.autoSaveId).toBe(
      undefined,
    );
    expectDataStyle(
      screen.getByTestId("resize-handle"),
      classicMainBodyStyles.resizeHandleDisabled,
    );
    expect(mocks.onResizeDragging).toBeNull();

    const panels = screen.getAllByTestId("panel");
    expect(panels[0]?.dataset.defaultSize).toBe("12.5");
    expect(panels[0]?.dataset.minSize).toBe("12.5");
    expect(panels[0]?.dataset.maxSize).toBe("12.5");
    expect(panels[0]?.dataset.flexGrow).toBe("0");
    expect(panels[0]?.dataset.flexBasis).toBe("200");
    expect(panels[0]?.dataset.minWidth).toBe("200");
    expect(panels[0]?.dataset.maxWidth).toBe("200");

    const sidebarChrome = document.querySelector<HTMLElement>(
      "[data-left-sidebar-chrome]",
    );
    expect(sidebarChrome?.style.width).toBe("200px");
    expect(sidebarChrome?.style.maxWidth).toBe("200px");

    const bodyRoot = screen.getByTestId("panel-group").parentElement;
    act(() => {
      mocks.onPanelLayout?.([24, 76]);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "12.5",
    );
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width")).toBe(
      "12.5%",
    );
  });

  it.each([
    ["settings", { state: { tab: "app" } }],
    ["calendar", {}],
    ["contacts", { state: { selected: null } }],
    ["automations", {}],
    ["templates", { state: { selectedMineId: null, selectedWebIndex: null } }],
  ] as const)(
    "leaves the %s chrome row back button to the sidebar header",
    (type, extraTabState) => {
      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: `slot-${type}`,
        type,
        ...extraTabState,
      };

      render(<ClassicMainBody />);

      expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    },
  );

  it("settles the startup left sidebar default against the rendered body width", async () => {
    let bodyWidth = 1000;
    const requestAnimationFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        callback(0);
        return 1;
      });
    const cancelAnimationFrame = vi
      .spyOn(window, "cancelAnimationFrame")
      .mockImplementation(() => {});
    const getBoundingClientRect = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        if (this.querySelector("[data-testid='panel-group']")) {
          return rectWithWidth(bodyWidth);
        }

        return rectWithWidth(0);
      });

    try {
      render(<ClassicMainBody />);

      await waitFor(() => {
        expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledWith(20);
      });

      const panels = screen.getAllByTestId("panel");
      expect(panels[0]?.dataset.defaultSize).toBe("20");
      expect(panels[0]?.dataset.minSize).toBe("20");
      expect(panels[0]?.dataset.maxSize).toBe("36");

      const bodyRoot = screen.getByTestId("panel-group").parentElement;
      expect(
        bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size"),
      ).toBe("20");
      expect(
        bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width"),
      ).toBe("20%");

      act(() => {
        mocks.onPanelLayout?.([12.5, 87.5]);
      });

      expect(
        bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size"),
      ).toBe("20");
      expect(
        bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width"),
      ).toBe("20%");

      bodyWidth = 800;
      fireEvent.resize(window);

      await waitFor(() => {
        expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledWith(25);
      });

      const resizedPanels = screen.getAllByTestId("panel");
      expect(resizedPanels[0]?.dataset.defaultSize).toBe("25");
      expect(resizedPanels[0]?.dataset.minSize).toBe("25");
      expect(resizedPanels[0]?.dataset.maxSize).toBe("45");
      expect(
        bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size"),
      ).toBe("25");
      expect(
        bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width"),
      ).toBe("25%");
    } finally {
      requestAnimationFrame.mockRestore();
      cancelAnimationFrame.mockRestore();
      getBoundingClientRect.mockRestore();
    }
  });

  it("syncs the default left sidebar width after the panel mounts", async () => {
    let bodyWidth = 1000;
    const requestAnimationFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        callback(0);
        return 1;
      });
    const cancelAnimationFrame = vi
      .spyOn(window, "cancelAnimationFrame")
      .mockImplementation(() => {});
    const getBoundingClientRect = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockImplementation(function (this: HTMLElement) {
        if (this.querySelector("[data-testid='panel-group']")) {
          return rectWithWidth(bodyWidth);
        }

        return rectWithWidth(0);
      });

    try {
      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: "slot-onboarding",
        type: "onboarding",
      };

      const { rerender } = render(<ClassicMainBody />);
      expect(
        screen
          .getAllByTestId("panel")
          .some(
            (panel) => panel.dataset.panelId === "classic-main-sidebar-left",
          ),
      ).toBe(false);

      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: "slot-home",
        type: "empty",
      };

      rerender(<ClassicMainBody />);
      bodyWidth = 1000;
      fireEvent.resize(window);

      await waitFor(() => {
        expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledWith(20);
      });
    } finally {
      requestAnimationFrame.mockRestore();
      cancelAnimationFrame.mockRestore();
      getBoundingClientRect.mockRestore();
    }
  });

  it("updates sidebar sizing during drag without rerendering tab content", () => {
    render(<ClassicMainBody />);

    const bodyRoot = screen.getByTestId("panel-group").parentElement;

    act(() => {
      mocks.onResizeDragging?.(true);
    });

    const renderCountAfterDragStart = mocks.tabContentRenderCount;

    act(() => {
      mocks.onPanelLayout?.([14, 86]);
      mocks.onPanelLayout?.([16, 84]);
      mocks.onPanelLayout?.([18, 82]);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "18",
    );
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width")).toBe(
      "18%",
    );
    expect(mocks.tabContentRenderCount).toBe(renderCountAfterDragStart);

    act(() => {
      mocks.onResizeDragging?.(false);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "18",
    );
  });

  it("updates the upcoming meeting badge without rerendering tab content", () => {
    mocks.leftsidebar.expanded = false;
    render(<ClassicMainBody />);
    const initialRenderCount = mocks.tabContentRenderCount;

    act(() => {
      mocks.setUpcomingMeetingStatus?.({
        itemKey: "event-standup",
        label: "In 1m",
        title: "Team standup",
      });
    });

    expect(
      screen.getByTestId("collapsed-sidebar-upcoming-meeting-badge"),
    ).toBeTruthy();
    expect(mocks.tabContentRenderCount).toBe(initialRenderCount);
  });

  it("keeps the note filter beside the new note button", () => {
    render(<ClassicMainBody />);

    const newNoteButton = screen.getByRole("button", { name: "New note" });
    const filterButton = screen.getByRole("button", { name: "Filter notes" });

    expect(filterButton.parentElement).toBe(newNoteButton.parentElement);
    expect(newNoteButton.compareDocumentPosition(filterButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("keeps near-equal sidebar size commits in sync with drag-time CSS variables", () => {
    render(<ClassicMainBody />);

    const bodyRoot = screen.getByTestId("panel-group").parentElement;

    act(() => {
      mocks.onResizeDragging?.(true);
      mocks.onPanelLayout?.([12.505, 87.495]);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "12.505",
    );

    act(() => {
      mocks.onResizeDragging?.(false);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "12.505",
    );
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width")).toBe(
      "12.505%",
    );
  });

  it("collapses the sidebar when the resize handle snaps below the threshold", () => {
    render(<ClassicMainBody />);

    const bodyRoot = screen.getByTestId("panel-group").parentElement;

    act(() => {
      mocks.onResizeDragging?.(true);
      mocks.onPanelLayout?.([0, 100]);
      mocks.onPanelCollapse?.();
    });

    expect(mocks.leftsidebar.setExpanded).toHaveBeenCalledWith(false);
    expect(mocks.leftsidebar.toggleExpanded).not.toHaveBeenCalled();
    expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledWith(12.5);
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "12.5",
    );
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width")).toBe(
      "12.5%",
    );
  });

  it("keeps the note content panel at least 500px wide", () => {
    mocks.currentTab = {
      active: true,
      id: "session-1",
      pinned: false,
      slotId: "slot-session",
      type: "sessions",
    };

    render(<ClassicMainBody />);

    const panels = screen.getAllByTestId("panel");
    expect(panels[1]?.dataset.minWidth).toBe("500");
  });

  it("keeps the empty content panel at least 500px wide", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-home",
      type: "empty",
    };

    render(<ClassicMainBody />);

    const panels = screen.getAllByTestId("panel");
    expect(panels[1]?.dataset.minWidth).toBe("500");
  });

  it("unmounts hidden sidebar content when the panel is collapsed", () => {
    mocks.leftsidebar.expanded = false;

    render(<ClassicMainBody />);

    const resizeHandle = screen.getByTestId("resize-handle");

    expectDataStyle(resizeHandle, classicMainBodyStyles.resizeHandleDisabled);
    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();

    const panels = screen.getAllByTestId("panel");
    expect(panels).toHaveLength(2);
    expect(panels[0]?.dataset.panelId).toBe("classic-main-sidebar-left");
    expect(panels[0]?.dataset.flexGrow).toBe("0");
    expect(panels[0]?.dataset.minWidth).toBe("0");
    expect(panels[0]?.dataset.maxWidth).toBe("0");
    expect(panels[0]?.dataset.transition).toBeUndefined();

    const sidebarContent = document.querySelector<HTMLElement>(
      "[data-left-sidebar-panel-content]",
    );
    expectStyle(sidebarContent, classicMainBodyStyles.sidebarContentCollapsed);
    expect(document.querySelector("[data-sidebar-timeline-header]")).toBeNull();
    expect(sidebarContent?.getAttribute("aria-hidden")).toBe("true");
    expect(sidebarContent?.hasAttribute("inert")).toBe(true);
  });

  it("resizes the collapsed sidebar panel before reopening it", () => {
    mocks.leftsidebar.expanded = false;

    const { rerender } = render(<ClassicMainBody />);
    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Show sidebar" }));

    expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledWith(12.5);
    expect(mocks.leftSidebarPanelHandle.expand).not.toHaveBeenCalled();
    expect(mocks.leftsidebar.toggleExpanded).toHaveBeenCalledTimes(1);

    mocks.leftsidebar.expanded = true;
    rerender(<ClassicMainBody />);

    expect(screen.getByTestId("classic-main-sidebar")).toBeTruthy();
  });

  it("keeps layout transitions disabled when resize is interrupted by collapse", () => {
    const { rerender } = render(<ClassicMainBody />);

    act(() => {
      mocks.onResizeDragging?.(true);
    });

    expect(screen.getAllByTestId("panel")[0]?.dataset.transition).toBe(
      undefined,
    );

    fireEvent.click(screen.getByRole("button", { name: "Hide sidebar" }));
    mocks.leftsidebar.expanded = false;
    rerender(<ClassicMainBody />);

    expect(
      screen.getAllByTestId("panel")[0]?.dataset.transition,
    ).toBeUndefined();
    expect(mocks.leftsidebar.toggleExpanded).toHaveBeenCalledTimes(1);
  });

  it("renders expanded sidebar controls in the sidebar layout", () => {
    render(<ClassicMainBody />);

    const sidebarChrome = document.querySelector<HTMLElement>(
      "[data-left-sidebar-chrome]",
    );
    const timelineHeader = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-header]",
    );
    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });
    const searchButton = screen.getByRole("button", { name: "Search" });

    expect(sidebarChrome).toBeNull();
    expect(timelineHeader).toBeInstanceOf(HTMLElement);
    expect(timelineHeader?.parentElement?.dataset.testid).toBe(
      "classic-main-sidebar",
    );
    expect(searchButton.parentElement).toBe(sidebarToggle.parentElement);

    const scroller = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-scroll]",
    );
    expect(scroller).toBeInstanceOf(HTMLElement);
    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });

    fireEvent.wheel(timelineHeader!, { deltaY: 80 });

    expect(scroller?.scrollTop).toBe(80);
  });

  it("does not reserve a sidebar panel during onboarding", () => {
    mocks.currentTab = { type: "onboarding" };

    render(<ClassicMainBody />);

    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
  });
});

function expectStyle(element: Element | null, sx: stylex.StyleXStyles) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(true);
  }
}

function expectDataStyle(element: HTMLElement, sx: stylex.StyleXStyles) {
  const classNames = new Set(element.dataset.className?.split(" "));
  const expectedClassNames = stylex.props(sx).className;
  expect(expectedClassNames).toBeTruthy();
  for (const className of expectedClassNames?.split(" ") ?? []) {
    expect(classNames.has(className)).toBe(true);
  }
}
