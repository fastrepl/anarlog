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
  openNew: vi.fn(),
  openSearch: vi.fn(),
  goBack: vi.fn(),
  goNext: vi.fn(),
  runEscapeShortcut: vi.fn(),
  toggleLeftSidebar: vi.fn(),
  isTauri: vi.fn(() => true),
  startDragging: vi.fn().mockResolvedValue(undefined),
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
  sidebarUpdateControl: {
    status: null as null | "available" | "downloading" | "ready" | "failed",
    version: null as string | null,
    progress: null as number | null,
    errorMessage: null as string | null,
    downloadStarting: false,
    installing: false,
    downloadUpdate: vi.fn(),
    installUpdate: vi.fn(),
  },
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
    startDragging: mocks.startDragging,
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
    ) : (
      <div data-testid="main-tab-content">{tab.type}</div>
    ),
}));

vi.mock("~/main/update-banner", () => ({
  SidebarTimelineUpdateButton: ({
    update,
  }: {
    update: { status: string | null; version: string | null };
  }) =>
    update.status && update.version ? (
      <button type="button" data-testid="sidebar-update-button" />
    ) : null,
  useDesktopUpdateControl: () => mocks.sidebarUpdateControl,
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

vi.mock("~/sidebar/toast", () => ({
  ToastArea: () => <div data-testid="toast-area" />,
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

import { ClassicMainBody } from "~/main/body";

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
    mocks.startDragging.mockClear();
    mocks.devtoolsPanelActionListeners = [];
    mocks.windowsCommands.devtoolsPanelHide.mockClear();
    mocks.windowsCommands.devtoolsPanelShow.mockClear();
    mocks.canGoBack = false;
    mocks.canGoNext = false;
    mocks.upcomingMeetingStatus = null;
    mocks.leftSidebarExpanded = true;
    mocks.sidebarUpdateControl.status = null;
    mocks.sidebarUpdateControl.version = null;
    mocks.sidebarUpdateControl.progress = null;
    mocks.sidebarUpdateControl.errorMessage = null;
    mocks.sidebarUpdateControl.downloadStarting = false;
    mocks.sidebarUpdateControl.installing = false;
    mocks.sidebarUpdateControl.downloadUpdate.mockClear();
    mocks.sidebarUpdateControl.installUpdate.mockClear();
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
    expect(sidebarToggle.parentElement?.className.split(" ")).toContain(
      "gap-0",
    );
    expect(sidebarToggle.parentElement?.className.split(" ")).not.toContain(
      "gap-0.5",
    );
    expect(chrome?.className).toContain("items-center");
    expect(chrome?.className).toContain("w-full");
    expect(chromeFrame).toBe(timelineHeader);
    expect(chromeFrame?.className).toContain("pr-1");
    expect(chromeFrame?.className).not.toContain("pr-3");
    expect(timelineHeader?.className).toContain("h-12");
    expect(timelineHeader?.className).not.toContain("absolute");
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
    expect(firstBodyChild?.className).toContain("min-h-0 flex-1");
    expect(firstBodyChild?.className).toContain("overflow-hidden");
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

    expect(screen.queryByTestId("sidebar-update-button")).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(sidebarToggle.className).toContain("pointer-events-auto");
    expect(topArea?.className).toContain("absolute");
    expect(topArea?.className).toContain("h-12");
    expect(topArea?.className).toContain("left-1");
    expect(topArea?.className).toContain("pointer-events-none");
    expect(contentRow?.className).toContain("min-h-0 flex-1");
    expect(contentRow?.className).toContain("overflow-hidden");
    expect(contentRow?.hasAttribute("data-tauri-drag-region")).toBe(false);
    expect(mocks.toggleLeftSidebar).toHaveBeenCalledTimes(1);
  });

  it("shows the update button in the expanded sidebar control group", () => {
    mocks.sidebarUpdateControl.status = "available";
    mocks.sidebarUpdateControl.version = "1.0.34";

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });
    const searchButton = screen.getByRole("button", { name: "Search" });
    const newNoteButton = screen.getByRole("button", { name: "New note" });
    const updateButton = screen.getByTestId("sidebar-update-button");
    const chrome = sidebarToggle.parentElement?.parentElement;
    const chromeFrame = chrome?.parentElement;
    const timelineHeader = document.querySelector<HTMLElement>(
      "[data-sidebar-timeline-header]",
    );

    expect(updateButton).toBeTruthy();
    expect(updateButton.parentElement).toBe(sidebarToggle.parentElement);
    expect(searchButton.compareDocumentPosition(newNoteButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(newNoteButton.compareDocumentPosition(updateButton)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(searchButton.parentElement).toBe(sidebarToggle.parentElement);
    expect(newNoteButton.parentElement).toBe(sidebarToggle.parentElement);
    expect(chrome?.className).toContain("items-center");
    expect(chromeFrame).toBe(timelineHeader);
    expect(chromeFrame?.className).toContain("pr-1");
    expect(chromeFrame?.className).not.toContain("pr-3");
    expect(
      within(sidebarToggle).queryByTestId("collapsed-sidebar-update-badge"),
    ).toBeNull();
  });

  it("shows ready updates in the expanded sidebar control group", () => {
    mocks.sidebarUpdateControl.status = "ready";
    mocks.sidebarUpdateControl.version = "1.0.34";

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });
    const updateButton = screen.getByTestId("sidebar-update-button");

    expect(updateButton).toBeTruthy();
    expect(updateButton.parentElement).toBe(sidebarToggle.parentElement);
  });

  it("shows an update badge on the collapsed sidebar toggle", () => {
    mocks.leftSidebarExpanded = false;
    mocks.sidebarUpdateControl.status = "available";
    mocks.sidebarUpdateControl.version = "1.0.34";

    render(<ClassicMainBody />);

    const sidebarToggle = screen.getByRole("button", { name: "Show sidebar" });

    fireEvent.click(sidebarToggle);

    expect(screen.queryByTestId("sidebar-update-button")).toBeNull();
    const badge = within(sidebarToggle).getByTestId(
      "collapsed-sidebar-update-badge",
    );

    expect(badge).toBeTruthy();
    expect(badge.className.split(" ")).toContain("bg-blue-500");
    expect(badge.className.split(" ")).not.toContain("bg-red-500");
    expect(mocks.toggleLeftSidebar).toHaveBeenCalledTimes(1);
  });

  it("shows a red upcoming meeting badge on the collapsed sidebar toggle", () => {
    mocks.leftSidebarExpanded = false;
    mocks.sidebarUpdateControl.status = "available";
    mocks.sidebarUpdateControl.version = "1.0.34";
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
    expect(badge.className.split(" ")).toContain("bg-red-500");
    expect(badge.className.split(" ")).not.toContain("bg-blue-500");
    expect(
      within(sidebarToggle).queryByTestId("collapsed-sidebar-update-badge"),
    ).toBeNull();
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
    expect(
      within(sidebarToggle).queryByTestId("collapsed-sidebar-update-badge"),
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
    expect(sidebarToggle.parentElement?.className.split(" ")).toContain(
      "gap-0",
    );
    expect(sidebarToggle.parentElement?.className.split(" ")).not.toContain(
      "gap-0.5",
    );
    expect(timelineHeader?.className).toContain("h-12");
    expect(timelineHeader?.className).not.toContain("absolute");
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "changelog",
    );
  });

  it.each(["calendar", "settings", "contacts", "templates"])(
    "runs the escape shortcut from the %s left chrome back button",
    (type) => {
      mocks.currentTab = {
        active: true,
        pinned: false,
        slotId: "slot-1",
        type,
      };

      render(<ClassicMainBody />);

      const backButton = screen.getByRole("button", { name: "Go back" });
      const topArea = backButton.parentElement?.parentElement;

      fireEvent.click(backButton);

      expect(screen.queryByTestId("timeline-update-banner")).toBeNull();
      expect(backButton.hasAttribute("disabled")).toBe(false);
      expect(topArea?.className).toContain("h-12");
      expect(topArea?.className).toContain("absolute");
      expect(mocks.goBack).not.toHaveBeenCalled();
      expect(mocks.runEscapeShortcut).toHaveBeenCalledTimes(1);
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
