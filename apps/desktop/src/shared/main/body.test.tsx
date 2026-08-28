import * as stylex from "@stylexjs/stylex";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNewNote: vi.fn(),
  openNew: vi.fn(),
  openSearch: vi.fn(),
  goBack: vi.fn(),
  goNext: vi.fn(),
  runEscapeShortcut: vi.fn(),
  toggleLeftSidebar: vi.fn(),
  isTauri: vi.fn(() => true),
  isFullscreen: vi.fn().mockResolvedValue(false),
  isMaximized: vi.fn().mockResolvedValue(false),
  platform: "macos" as "linux" | "macos" | "windows",
  resizeListeners: [] as Array<() => void>,
  startDragging: vi.fn().mockResolvedValue(undefined),
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
  devtoolsPanelActionListeners: [] as Array<
    (event: { payload: { action: string } }) => void
  >,
  windowsCommands: {
    devtoolsPanelHide: vi.fn(async () => ({ status: "ok" as const })),
    devtoolsPanelShow: vi.fn(async () => ({ status: "ok" as const })),
  },
  canGoBack: false,
  canGoNext: false,
  upcomingMeetingStatus: null as null | {
    itemKey: string;
    label: string;
    title: string;
  },
  leftSidebarExpanded: true,
  currentTab: {
    active: true,
    pinned: false,
    slotId: "slot-1",
    type: "empty",
  } as null | {
    active: boolean;
    id?: string;
    pinned: boolean;
    slotId: string;
    type: string;
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: mocks.isTauri,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isFullscreen: mocks.isFullscreen,
    isMaximized: mocks.isMaximized,
    onResized: vi.fn(async (listener: () => void) => {
      mocks.resizeListeners.push(listener);
      return () => {
        mocks.resizeListeners = mocks.resizeListeners.filter(
          (candidate) => candidate !== listener,
        );
      };
    }),
    startDragging: mocks.startDragging,
    toggleMaximize: mocks.toggleMaximize,
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@anlg/plugin-windows", () => ({
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

vi.mock("~/main/useShortcuts", () => ({
  useClassicMainShortcuts: vi.fn(() => ({
    runEscapeShortcut: mocks.runEscapeShortcut,
  })),
}));

vi.mock("~/main/tab-content", () => ({
  ClassicMainTabContent: ({ tab }: { tab: { type: string } }) =>
    tab.type === "sessions" ? (
      <div data-testid="main-tab-content">
        <input aria-label="Session title" />
      </div>
    ) : tab.type === "empty" ? (
      <div data-testid="main-tab-content">
        <div data-tauri-drag-region data-testid="native-main-tab-drag-region">
          <span data-testid="native-main-tab-drag-target">{tab.type}</span>
        </div>
      </div>
    ) : (
      <div data-testid="main-tab-content">{tab.type}</div>
    ),
}));

vi.mock("~/sidebar/note-filter-menu", () => ({
  SidebarNoteFilterMenu: () => <button type="button">Filter notes</button>,
}));

vi.mock("~/sidebar/timeline/upcoming-meeting", () => ({
  useSidebarUpcomingMeetingStatus: () => mocks.upcomingMeetingStatus,
}));

vi.mock("~/main/shell-sidebar", () => ({
  ClassicMainSidebar: ({
    timelineHeader,
  }: {
    timelineHeader?: React.ReactNode;
  }) => (
    <div data-testid="main-sidebar">
      {timelineHeader}
      <div data-sidebar-timeline-scroll />
    </div>
  ),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: {
      expanded: mocks.leftSidebarExpanded,
      toggleExpanded: mocks.toggleLeftSidebar,
    },
  }),
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({
    open: mocks.openSearch,
  }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.createNewNote,
}));

vi.mock("~/store/zustand/tabs", () => ({
  uniqueIdfromTab: vi.fn(() => "empty-slot"),
  useTabs: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      tabs: [{ active: true, pinned: false, slotId: "slot-1", type: "empty" }],
      currentTab: mocks.currentTab,
      canGoBack: mocks.canGoBack,
      canGoNext: mocks.canGoNext,
      goBack: mocks.goBack,
      goNext: mocks.goNext,
      openNew: mocks.openNew,
    }),
  ),
}));

import { classicMainBodyStyles, ClassicMainBody } from "~/main/body";

describe("ClassicMainBody", () => {
  beforeEach(() => {
    mocks.createNewNote.mockClear();
    mocks.openNew.mockClear();
    mocks.openSearch.mockClear();
    mocks.goBack.mockClear();
    mocks.goNext.mockClear();
    mocks.runEscapeShortcut.mockClear();
    mocks.toggleLeftSidebar.mockClear();
    mocks.isTauri.mockReturnValue(true);
    mocks.isFullscreen.mockReset();
    mocks.isFullscreen.mockResolvedValue(false);
    mocks.isMaximized.mockReset();
    mocks.isMaximized.mockResolvedValue(false);
    mocks.platform = "macos";
    mocks.resizeListeners = [];
    mocks.startDragging.mockClear();
    mocks.toggleMaximize.mockClear();
    mocks.devtoolsPanelActionListeners = [];
    mocks.windowsCommands.devtoolsPanelHide.mockClear();
    mocks.windowsCommands.devtoolsPanelShow.mockClear();
    mocks.canGoBack = false;
    mocks.canGoNext = false;
    mocks.upcomingMeetingStatus = null;
    mocks.leftSidebarExpanded = true;
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "empty",
    };
  });

  afterEach(() => {
    cleanup();
  });

  it("renders sidebar timeline chrome and current tab content", () => {
    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });
    const searchButton = screen.getByRole("button", { name: "Search" });
    const newNoteButton = screen.getByRole("button", { name: "New note" });
    const chrome = sidebarToggle.parentElement?.parentElement;
    const chromeFrame = chrome?.parentElement;
    const timelineHeader = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-header]",
    );

    fireEvent.click(searchButton);
    fireEvent.click(newNoteButton);

    expect(screen.getByTestId("main-sidebar")).toBeTruthy();
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "empty",
    );
    expect(screen.queryByTestId("timeline-update-banner")).toBeNull();
    expect(sidebarToggle.parentElement?.className).toBeTruthy();
    expect(chrome?.className).toBeTruthy();
    expect(chromeFrame).toBe(timelineHeader);
    expectStyle(timelineHeader, classicMainBodyStyles.timelineHeader);
    expect(chrome?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(
      sidebarToggle.parentElement?.hasAttribute("data-tauri-drag-region"),
    ).toBe(true);
    expect(sidebarToggle.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(searchButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(newNoteButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(sidebarToggle.compareDocumentPosition(searchButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(searchButton.compareDocumentPosition(newNoteButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(mocks.openSearch).toHaveBeenCalledTimes(1);
    expect(mocks.createNewNote).toHaveBeenCalledTimes(1);
  });

  it("does not reserve top shell chrome for onboarding", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "onboarding",
    };

    const { container } = render(<ClassicMainBody />);
    const body = container.firstElementChild;
    const firstBodyChild = body?.firstElementChild;

    expect(screen.queryByTestId("timeline-update-banner")).toBeNull();
    expect(screen.queryByTestId("toast-area")).toBeNull();
    expect(firstBodyChild?.className).toBeTruthy();
    expect(firstBodyChild?.hasAttribute("data-tauri-drag-region")).toBe(false);
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "onboarding",
    );
  });

  it("expands the main area to the full window when the sidebar is collapsed", () => {
    mocks.leftSidebarExpanded = false;

    const { container } = render(<ClassicMainBody />);
    const body = container.firstElementChild;
    const contentRow = body?.lastElementChild;
    const sidebarToggle = screen.getByRole("button", { name: "Show sidebar" });
    const chrome = sidebarToggle.parentElement?.parentElement;
    const topArea = chrome?.parentElement?.parentElement;

    fireEvent.click(sidebarToggle);

    expect(screen.queryByRole("button", { name: "Filter notes" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(sidebarToggle.className).toBeTruthy();
    expectStyle(topArea, classicMainBodyStyles.collapsedSidebarChrome);
    expect(contentRow?.className).toBeTruthy();
    expect(contentRow?.hasAttribute("data-tauri-drag-region")).toBe(false);
    expect(mocks.toggleLeftSidebar).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["expanded", true, "Hide sidebar"],
    ["collapsed", false, "Show sidebar"],
  ])(
    "removes the window controls gutter in fullscreen with the sidebar %s",
    async (_state, expanded, toggleLabel) => {
      mocks.leftSidebarExpanded = expanded;

      render(<ClassicMainBody />);

      const sidebarToggle = screen.getByRole("button", { name: toggleLabel });
      const chromeFrame = expanded
        ? document.querySelector<HTMLElement>("[data-sidebar-timeline-header]")
        : sidebarToggle.parentElement?.parentElement?.parentElement;

      expectStyle(chromeFrame, classicMainBodyStyles.windowControlsGutter);

      mocks.isFullscreen.mockResolvedValue(true);
      act(() => {
        for (const listener of mocks.resizeListeners) {
          listener();
        }
      });

      await waitFor(() => {
        expectStyle(chromeFrame, classicMainBodyStyles.defaultGutter);
      });
      expectNotStyle(chromeFrame, classicMainBodyStyles.windowControlsGutter);
    },
  );

  it.each([
    ["expanded", true, "Hide sidebar"],
    ["collapsed", false, "Show sidebar"],
  ])(
    "keeps the window controls gutter while maximized with the sidebar %s",
    async (_state, expanded, toggleLabel) => {
      mocks.leftSidebarExpanded = expanded;
      mocks.isMaximized.mockResolvedValue(true);

      render(<ClassicMainBody />);

      const sidebarToggle = screen.getByRole("button", { name: toggleLabel });
      const chromeFrame = expanded
        ? document.querySelector<HTMLElement>("[data-sidebar-timeline-header]")
        : sidebarToggle.parentElement?.parentElement?.parentElement;

      act(() => {
        for (const listener of mocks.resizeListeners) {
          listener();
        }
      });

      await waitFor(() => {
        expect(mocks.isFullscreen).toHaveBeenCalled();
      });
      expectStyle(chromeFrame, classicMainBodyStyles.windowControlsGutter);
      expectNotStyle(chromeFrame, classicMainBodyStyles.defaultGutter);
    },
  );

  it.each([
    ["windows", "expanded", true],
    ["windows", "collapsed", false],
    ["linux", "expanded", true],
    ["linux", "collapsed", false],
  ] as const)(
    "leaves the sidebar toggle to the title bar on %s while %s",
    async (runtimePlatform, _state, expanded) => {
      mocks.leftSidebarExpanded = expanded;
      mocks.platform = runtimePlatform;

      render(<ClassicMainBody />);

      const chromeFrame = expanded
        ? document.querySelector<HTMLElement>("[data-sidebar-timeline-header]")
        : document.querySelector<HTMLElement>(
            "[data-left-sidebar-chrome] > div",
          );

      await waitFor(() => {
        expectStyle(chromeFrame, classicMainBodyStyles.defaultGutter);
      });
      expect(
        screen.queryByRole("button", {
          name: expanded ? "Hide sidebar" : "Show sidebar",
        }),
      ).toBeNull();
      expectNotStyle(chromeFrame, classicMainBodyStyles.windowControlsGutter);
      expect(mocks.isFullscreen).not.toHaveBeenCalled();
      expect(mocks.resizeListeners).toHaveLength(0);
    },
  );

  it("shows the note filter beside the new note button", () => {
    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });
    const searchButton = screen.getByRole("button", { name: "Search" });
    const newNoteButton = screen.getByRole("button", { name: "New note" });
    const filterButton = screen.getByRole("button", { name: "Filter notes" });
    const chrome = sidebarToggle.parentElement?.parentElement;
    const chromeFrame = chrome?.parentElement;
    const timelineHeader = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-header]",
    );

    expect(filterButton).toBeTruthy();
    expect(filterButton.parentElement).toBe(sidebarToggle.parentElement);
    expect(searchButton.compareDocumentPosition(newNoteButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(newNoteButton.compareDocumentPosition(filterButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(searchButton.parentElement).toBe(sidebarToggle.parentElement);
    expect(newNoteButton.parentElement).toBe(sidebarToggle.parentElement);
    expect(chrome?.className).toBeTruthy();
    expect(chromeFrame).toBe(timelineHeader);
    expectStyle(chromeFrame, classicMainBodyStyles.timelineHeader);
  });

  it("hides the note filter while the sidebar is collapsed", () => {
    mocks.leftSidebarExpanded = false;

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Show sidebar" });

    fireEvent.click(sidebarToggle);

    expect(screen.queryByRole("button", { name: "Filter notes" })).toBeNull();
    expect(mocks.toggleLeftSidebar).toHaveBeenCalledTimes(1);
  });

  it("shows a red upcoming meeting badge on the collapsed sidebar toggle", () => {
    mocks.leftSidebarExpanded = false;
    mocks.upcomingMeetingStatus = {
      itemKey: "session-upcoming",
      label: "Starts in 3m",
      title: "Devtool design sync",
    };

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Show sidebar" });
    const badge = within(sidebarToggle).getByTestId(
      "collapsed-sidebar-upcoming-meeting-badge",
    );

    expect(badge).toBeTruthy();
    expect(badge.className).toBeTruthy();
  });

  it("hides the red upcoming meeting badge when that note is already open", () => {
    mocks.leftSidebarExpanded = false;
    mocks.currentTab = {
      active: true,
      id: "upcoming",
      pinned: false,
      slotId: "slot-1",
      type: "sessions",
    };
    mocks.upcomingMeetingStatus = {
      itemKey: "session-upcoming",
      label: "Starts in 3m",
      title: "Devtool design sync",
    };

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Show sidebar" });

    expect(
      within(sidebarToggle).queryByTestId(
        "collapsed-sidebar-upcoming-meeting-badge",
      ),
    ).toBeNull();
  });

  it("keeps sidebar chrome for changelog tabs", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "changelog",
    };

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });
    const chrome = sidebarToggle.parentElement?.parentElement;
    const timelineHeader = chrome?.parentElement;

    expect(screen.queryByTestId("timeline-update-banner")).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(sidebarToggle.parentElement?.className).toBeTruthy();
    expectStyle(timelineHeader, classicMainBodyStyles.timelineHeader);
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "changelog",
    );
  });

  it.each(["calendar", "settings", "contacts", "templates"])(
    "renders no chrome back button over the %s sidebar header",
    (type) => {
      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: "slot-1",
        type,
      };

      render(<ClassicMainBody />);

      expect(screen.queryByTestId("timeline-update-banner")).toBeNull();
      expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
      expect(mocks.runEscapeShortcut).not.toHaveBeenCalled();
    },
  );

  it("starts window dragging from the top 48px of the main area", () => {
    render(<ClassicMainBody />);

    const mainContent = screen.getByTestId("main-tab-content");

    fireEvent.pointerDown(mainContent, {
      button: 0,
      clientX: 12,
      clientY: 12,
      pointerId: 1,
    });
    fireEvent.pointerMove(mainContent, {
      clientX: 20,
      clientY: 12,
      pointerId: 1,
    });

    expect(mocks.startDragging).toHaveBeenCalledTimes(1);
  });

  it("toggles window maximization from the top 48px of the main area", () => {
    render(<ClassicMainBody />);

    const mainContent = screen.getByTestId("main-tab-content");

    fireEvent.doubleClick(mainContent, {
      button: 0,
      clientX: 12,
      clientY: 12,
    });

    expect(mocks.toggleMaximize).toHaveBeenCalledTimes(1);
  });

  it("leaves exact native drag-region double-clicks to Tauri", () => {
    render(<ClassicMainBody />);

    const nativeDragRegion = screen.getByTestId("native-main-tab-drag-region");

    fireEvent.doubleClick(nativeDragRegion, {
      button: 0,
      clientX: 12,
      clientY: 12,
    });

    expect(mocks.toggleMaximize).not.toHaveBeenCalled();
  });

  it("toggles maximization from children of a native drag region", () => {
    render(<ClassicMainBody />);

    const nativeDragTarget = screen.getByTestId("native-main-tab-drag-target");

    fireEvent.doubleClick(nativeDragTarget, {
      button: 0,
      clientX: 12,
      clientY: 12,
    });

    expect(mocks.toggleMaximize).toHaveBeenCalledTimes(1);
  });

  it("does not start window dragging from an input in the top drag strip", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "sessions",
    };

    render(<ClassicMainBody />);

    const titleInput = screen.getByRole("textbox", { name: "Session title" });

    fireEvent.pointerDown(titleInput, {
      button: 0,
      clientX: 240,
      clientY: 12,
      pointerId: 1,
    });
    fireEvent.pointerMove(titleInput, {
      clientX: 248,
      clientY: 12,
      pointerId: 1,
    });

    expect(mocks.startDragging).not.toHaveBeenCalled();
  });

  it("does not toggle window maximization from an input in the top drag strip", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "sessions",
    };

    render(<ClassicMainBody />);

    const titleInput = screen.getByRole("textbox", { name: "Session title" });

    fireEvent.doubleClick(titleInput, {
      button: 0,
      clientX: 240,
      clientY: 12,
    });

    expect(mocks.toggleMaximize).not.toHaveBeenCalled();
  });

  it("does not start window dragging below the main area drag strip", () => {
    render(<ClassicMainBody />);

    const mainContent = screen.getByTestId("main-tab-content");

    fireEvent.pointerDown(mainContent, {
      button: 0,
      clientX: 12,
      clientY: 56,
      pointerId: 1,
    });
    fireEvent.pointerMove(mainContent, {
      clientX: 20,
      clientY: 56,
      pointerId: 1,
    });

    expect(mocks.startDragging).not.toHaveBeenCalled();
  });

  it("does not toggle window maximization below the main area drag strip", () => {
    render(<ClassicMainBody />);

    const mainContent = screen.getByTestId("main-tab-content");

    fireEvent.doubleClick(mainContent, {
      button: 0,
      clientX: 12,
      clientY: 56,
    });

    expect(mocks.toggleMaximize).not.toHaveBeenCalled();
  });

  it("renders the shell while the initial tab is still loading", async () => {
    const { useTabs } = await import("~/store/zustand/tabs");

    vi.mocked(useTabs).mockImplementationOnce(((
      selector: (state: unknown) => unknown,
    ) =>
      selector({
        tabs: [],
        currentTab: null,
      })) as typeof useTabs);

    const { container } = render(<ClassicMainBody />);
    const view = within(container);

    expect(view.getByTestId("main-sidebar")).toBeTruthy();
    expect(view.queryByTestId("main-tab-content")).toBeNull();
  });
});

function expectStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(true);
  }
}

function expectNotStyle(
  element: Element | null | undefined,
  sx: stylex.StyleXStyles,
) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(false);
  }
}
