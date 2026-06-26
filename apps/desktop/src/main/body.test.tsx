import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: {
    active: true,
    pinned: false,
    slotId: "slot-home",
    type: "empty",
  } as ({ type: string } & Record<string, unknown>) | null,
  leftsidebar: {
    expanded: true,
    toggleExpanded: vi.fn(),
  },
  onPanelLayout: null as null | ((sizes: number[]) => void),
  onResizeDragging: null as null | ((isDragging: boolean) => void),
  tabContentRenderCount: 0,
  devtoolsPanelActionListeners: [] as Array<
    (event: { payload: { action: string } }) => void
  >,
  windowsCommands: {
    devtoolsPanelHide: vi.fn(async () => ({ status: "ok" as const })),
    devtoolsPanelShow: vi.fn(async () => ({ status: "ok" as const })),
  },
}));

vi.mock("@hypr/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    autoSaveId,
    children,
    className,
    dir,
    direction,
    onLayout,
  }: {
    autoSaveId?: string;
    children: React.ReactNode;
    className?: string;
    dir?: string;
    direction: string;
    onLayout?: (sizes: number[]) => void;
  }) => {
    mocks.onPanelLayout = onLayout ?? null;

    return (
      <div
        data-auto-save-id={autoSaveId}
        data-class-name={className}
        data-dir={dir}
        data-direction={direction}
        data-testid="panel-group"
      >
        {children}
      </div>
    );
  },
  ResizablePanel: ({
    children,
    className,
    defaultSize,
    id,
    maxSize,
    minSize,
    order,
    style,
  }: {
    children: React.ReactNode;
    className?: string;
    defaultSize?: number;
    id?: string;
    maxSize?: number;
    minSize?: number;
    order?: number;
    style?: React.CSSProperties;
  }) => (
    <div
      data-class-name={className}
      data-default-size={defaultSize}
      data-panel-id={id}
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
  ),
  ResizableHandle: ({
    className,
    onDragging,
  }: {
    className?: string;
    onDragging?: (isDragging: boolean) => void;
  }) => {
    mocks.onResizeDragging = onDragging ?? null;

    return <div data-class-name={className} data-testid="resize-handle" />;
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: mocks.windowsCommands,
  events: {
    devtoolsPanelAction: {
      listen: vi.fn(
        async (listener: (event: { payload: { action: string } }) => void) => {
          mocks.devtoolsPanelActionListeners.push(listener);
          return () => {
            mocks.devtoolsPanelActionListeners =
              mocks.devtoolsPanelActionListeners.filter(
                (candidate) => candidate !== listener,
              );
          };
        },
      ),
    },
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  uniqueIdfromTab: (tab: { type: string }) => tab.type,
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("./shell-sidebar", () => ({
  ClassicMainSidebar: ({ forceMount = false }: { forceMount?: boolean }) =>
    (forceMount || mocks.leftsidebar.expanded) &&
    mocks.currentTab?.type !== "onboarding" ? (
      <aside data-testid="classic-main-sidebar">
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

vi.mock("./update-banner", () => ({
  SidebarTimelineUpdateButton: () => <button type="button">Update</button>,
  useDesktopUpdateControl: () => ({ status: null, version: null }),
}));

vi.mock("./useTabsShortcuts", () => ({
  useClassicMainTabsShortcuts: () => ({ runEscapeShortcut: vi.fn() }),
}));

vi.mock("~/session/components/bottom-accessory/global-live", () => ({
  GlobalLiveTranscriptAccessory: ({
    children,
  }: {
    children: React.ReactNode;
  }) => <div data-testid="global-live-accessory">{children}</div>,
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({ open: vi.fn() }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
}));

vi.mock("~/sidebar/timeline/upcoming-meeting", () => ({
  useSidebarUpcomingMeetingStatus: () => null,
}));

import { ClassicMainBody } from "./body";

describe("ClassicMainBody", () => {
  beforeEach(() => {
    cleanup();
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
    mocks.leftsidebar.toggleExpanded.mockClear();
    mocks.onPanelLayout = null;
    mocks.onResizeDragging = null;
    mocks.tabContentRenderCount = 0;
    mocks.devtoolsPanelActionListeners = [];
    mocks.windowsCommands.devtoolsPanelHide.mockClear();
    mocks.windowsCommands.devtoolsPanelShow.mockClear();
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
    expect(screen.getByTestId("classic-main-sidebar")).toBeTruthy();
    expect(screen.getByTestId("resize-handle").dataset.className).toContain(
      "after:w-2",
    );
    expect(screen.getByTestId("resize-handle").dataset.className).toContain(
      "w-1",
    );

    const panels = screen.getAllByTestId("panel");
    expect(panels).toHaveLength(2);
    expect(panels[0]?.dataset.panelId).toBe("classic-main-sidebar-left");
    expect(panels[0]?.dataset.order).toBe("1");
    expect(panels[0]?.dataset.defaultSize).toBe("12.5");
    expect(panels[0]?.dataset.minSize).toBe("12.5");
    expect(panels[0]?.dataset.maxSize).toBe("22.5");
    expect(panels[0]?.dataset.flexGrow).toBe("var(--left-sidebar-panel-size)");
    expect(panels[0]?.dataset.minWidth).toBe("200");
    expect(panels[0]?.dataset.maxWidth).toBe("360");
    expect(panels[0]?.dataset.transition).toContain("flex-grow");
    expect(panels[1]?.dataset.panelId).toBe("classic-main-content");
    expect(panels[1]?.dataset.order).toBe("2");

    const sidebarContent = document.querySelector<HTMLElement>(
      "[data-left-sidebar-panel-content]",
    );
    const sidebarChrome = document.querySelector<HTMLElement>(
      "[data-left-sidebar-chrome]",
    );

    expect(sidebarContent?.className).toContain("translate-x-0");
    expect(sidebarContent?.getAttribute("aria-hidden")).toBe("false");
    expect(sidebarChrome?.style.width).toBe("var(--left-sidebar-panel-width)");
    expect(sidebarChrome?.style.minWidth).toBe("200px");
    expect(sidebarChrome?.style.maxWidth).toBe("360px");
    expect(sidebarChrome?.className).not.toContain("w-[200px]");

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

  it("collapses the sidebar panel without unmounting the animated shell", () => {
    mocks.leftsidebar.expanded = false;

    render(<ClassicMainBody />);

    const resizeHandle = screen.getByTestId("resize-handle");

    expect(resizeHandle.dataset.className).toContain("pointer-events-none");
    expect(resizeHandle.dataset.className).toContain("w-0");
    expect(screen.getByTestId("classic-main-sidebar")).toBeTruthy();

    const panels = screen.getAllByTestId("panel");
    expect(panels).toHaveLength(2);
    expect(panels[0]?.dataset.panelId).toBe("classic-main-sidebar-left");
    expect(panels[0]?.dataset.flexGrow).toBe("0");
    expect(panels[0]?.dataset.minWidth).toBe("0");
    expect(panels[0]?.dataset.maxWidth).toBe("0");
    expect(panels[0]?.dataset.transition).toContain("flex-grow");

    const sidebarContent = document.querySelector<HTMLElement>(
      "[data-left-sidebar-panel-content]",
    );
    expect(sidebarContent?.className).toContain("-translate-x-3");
    expect(sidebarContent?.className).toContain("opacity-0");
    expect(sidebarContent?.getAttribute("aria-hidden")).toBe("true");
    expect(sidebarContent?.hasAttribute("inert")).toBe(true);
  });

  it("restores sidebar transitions when a resize is interrupted by collapse", () => {
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

    expect(screen.getAllByTestId("panel")[0]?.dataset.transition).toContain(
      "flex-grow",
    );
    expect(mocks.leftsidebar.toggleExpanded).toHaveBeenCalledTimes(1);
  });

  it("shows the devtools button until the panel opens, then restores it when closed", async () => {
    render(<ClassicMainBody />);

    const searchButton = screen.getByRole("button", { name: "Search" });
    const newNoteButton = screen.getByRole("button", { name: "New note" });
    const devtoolsButton = screen.getByRole("button", {
      name: "Show devtools panel",
    });

    expect(searchButton.compareDocumentPosition(newNoteButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(newNoteButton.compareDocumentPosition(devtoolsButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(devtoolsButton.parentElement).toBe(newNoteButton.parentElement);

    fireEvent.click(devtoolsButton);

    expect(mocks.windowsCommands.devtoolsPanelShow).toHaveBeenCalledTimes(1);
    expect(mocks.windowsCommands.devtoolsPanelHide).not.toHaveBeenCalled();
    expect(
      screen.getByRole("button", { name: "Show devtools panel" }),
    ).toBeTruthy();

    act(() => {
      for (const listener of mocks.devtoolsPanelActionListeners) {
        listener({ payload: { action: "panel:opened" } });
      }
    });

    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: "Show devtools panel" }),
      ).toBeNull();
    });

    act(() => {
      for (const listener of mocks.devtoolsPanelActionListeners) {
        listener({ payload: { action: "panel:closed" } });
      }
    });

    expect(
      await screen.findByRole("button", { name: "Show devtools panel" }),
    ).toBeTruthy();
  });

  it("hides the devtools button when the native panel is opened outside the sidebar", async () => {
    render(<ClassicMainBody />);

    expect(
      screen.getByRole("button", { name: "Show devtools panel" }),
    ).toBeTruthy();

    await waitFor(() => {
      expect(mocks.devtoolsPanelActionListeners).toHaveLength(1);
    });

    act(() => {
      for (const listener of mocks.devtoolsPanelActionListeners) {
        listener({ payload: { action: "panel:opened" } });
      }
    });

    expect(
      screen.queryByRole("button", { name: "Show devtools panel" }),
    ).toBeNull();

    act(() => {
      for (const listener of mocks.devtoolsPanelActionListeners) {
        listener({ payload: { action: "panel:closed" } });
      }
    });

    expect(
      await screen.findByRole("button", { name: "Show devtools panel" }),
    ).toBeTruthy();
  });

  it("routes wheel gestures from sidebar chrome into the timeline scroller", () => {
    render(<ClassicMainBody />);

    const sidebarChrome = document.querySelector<HTMLElement>(
      "[data-left-sidebar-chrome]",
    );
    const timelineScroller = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-scroll]",
    );

    expect(sidebarChrome).toBeInstanceOf(HTMLElement);
    expect(timelineScroller).toBeInstanceOf(HTMLElement);

    Object.defineProperty(timelineScroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(timelineScroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });

    fireEvent.wheel(sidebarChrome!, { deltaY: 96 });

    expect(timelineScroller!.scrollTop).toBe(96);
  });

  it("does not reserve a sidebar panel during onboarding", () => {
    mocks.currentTab = { type: "onboarding" };

    render(<ClassicMainBody />);

    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
  });
});
