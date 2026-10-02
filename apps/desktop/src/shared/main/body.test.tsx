import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNewNote: vi.fn(),
  openSearch: vi.fn(),
  toggleLeftSidebar: vi.fn(),
  isTauri: vi.fn(() => true),
  isFullscreen: vi.fn().mockResolvedValue(false),
  platform: "macos" as "linux" | "macos" | "windows",
  startDragging: vi.fn().mockResolvedValue(undefined),
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
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
    onResized: vi.fn(async () => () => {}),
    startDragging: mocks.startDragging,
    toggleMaximize: mocks.toggleMaximize,
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("~/main/useShortcuts", () => ({
  useClassicMainShortcuts: vi.fn(),
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
  SidebarNoteFilterMenu: () => <button type="button">Sort notes</button>,
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
    selector({ currentTab: mocks.currentTab }),
  ),
}));

import { ClassicMainBody } from "~/main/body";

describe("ClassicMainBody", () => {
  beforeEach(() => {
    mocks.createNewNote.mockClear();
    mocks.openSearch.mockClear();
    mocks.toggleLeftSidebar.mockClear();
    mocks.isTauri.mockReturnValue(true);
    mocks.isFullscreen.mockReset();
    mocks.isFullscreen.mockResolvedValue(false);
    mocks.platform = "macos";
    mocks.startDragging.mockClear();
    mocks.toggleMaximize.mockClear();
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

    fireEvent.click(screen.getByRole("button", { name: "Search" }));
    fireEvent.click(screen.getByRole("button", { name: "New note" }));

    expect(screen.getByTestId("main-sidebar")).toBeTruthy();
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "empty",
    );
    expect(mocks.openSearch).toHaveBeenCalledTimes(1);
    expect(mocks.createNewNote).toHaveBeenCalledTimes(1);
  });

  it("shows the note filter while the sidebar is expanded", () => {
    render(<ClassicMainBody />);

    expect(screen.getByRole("button", { name: "Sort notes" })).toBeTruthy();
  });

  it("toggles the collapsed sidebar and hides the note filter", () => {
    mocks.leftSidebarExpanded = false;

    render(<ClassicMainBody />);

    fireEvent.click(screen.getByRole("button", { name: "Show sidebar" }));

    expect(mocks.toggleLeftSidebar).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Sort notes" })).toBeNull();
  });

  it.each([
    ["shows", undefined, true],
    ["hides when that note is already open", "upcoming", false],
  ] as const)(
    "%s the upcoming meeting badge on the collapsed sidebar toggle",
    (_description, currentTabId, isBadgePresent) => {
      mocks.leftSidebarExpanded = false;
      mocks.upcomingMeetingStatus = {
        itemKey: "session-upcoming",
        label: "Starts in 3m",
        title: "Devtool design sync",
      };

      if (currentTabId) {
        mocks.currentTab = {
          active: true,
          id: currentTabId,
          pinned: false,
          slotId: "slot-1",
          type: "sessions",
        };
      }

      render(<ClassicMainBody />);

      const sidebarToggle = screen.getByRole("button", {
        name: "Show sidebar",
      });
      const badge = within(sidebarToggle).queryByTestId(
        "collapsed-sidebar-upcoming-meeting-badge",
      );

      expect(Boolean(badge)).toBe(isBadgePresent);
    },
  );

  it.each([
    ["main-tab-content", 12, "empty", 1],
    ["Session title textbox", 12, "sessions", 0],
    ["main-tab-content", 56, "empty", 0],
  ] as const)(
    "starts window dragging from %s at y=%i on the %s tab",
    (target, clientY, tabType, expectedCalls) => {
      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: "slot-1",
        type: tabType,
      };

      render(<ClassicMainBody />);

      const dragTarget = resolveDragTarget(target);

      fireEvent.pointerDown(dragTarget, {
        button: 0,
        clientX: 12,
        clientY,
        pointerId: 1,
      });
      fireEvent.pointerMove(dragTarget, {
        clientX: 20,
        clientY,
        pointerId: 1,
      });

      expect(mocks.startDragging).toHaveBeenCalledTimes(expectedCalls);
    },
  );

  it.each([
    ["main-tab-content", 12, "empty", 1],
    ["native-main-tab-drag-region", 12, "empty", 0],
    ["native-main-tab-drag-target", 12, "empty", 1],
    ["Session title textbox", 12, "sessions", 0],
    ["main-tab-content", 56, "empty", 0],
  ] as const)(
    "toggles window maximization from %s at y=%i on the %s tab",
    (target, clientY, tabType, expectedCalls) => {
      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: "slot-1",
        type: tabType,
      };

      render(<ClassicMainBody />);

      const dragTarget = resolveDragTarget(target);

      fireEvent.doubleClick(dragTarget, {
        button: 0,
        clientX: 12,
        clientY,
      });

      expect(mocks.toggleMaximize).toHaveBeenCalledTimes(expectedCalls);
    },
  );

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

function resolveDragTarget(
  target:
    | "main-tab-content"
    | "native-main-tab-drag-region"
    | "native-main-tab-drag-target"
    | "Session title textbox",
) {
  if (target === "Session title textbox") {
    return screen.getByRole("textbox", { name: "Session title" });
  }

  return screen.getByTestId(target);
}
