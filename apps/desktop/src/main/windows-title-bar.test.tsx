import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { resetSidebarNotes, useSidebarNotes } from "~/sidebar/note-filter";

const mocks = vi.hoisted(() => ({
  close: vi.fn().mockResolvedValue(undefined),
  createNewNote: vi.fn(),
  isFullscreen: vi.fn().mockResolvedValue(false),
  isMaximized: vi.fn().mockResolvedValue(false),
  minimize: vi.fn().mockResolvedValue(undefined),
  onResized: vi.fn().mockResolvedValue(vi.fn()),
  openNew: vi.fn(),
  openNoteDialog: vi.fn(),
  openUrl: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  setFullscreen: vi.fn().mockResolvedValue(undefined),
  toggleExpanded: vi.fn(),
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
  currentTab: { type: "empty" } as { id?: string; type: string },
  leftSidebarExpanded: true,
  platform: "windows",
  upcomingMeetingStatus: null as null | { itemKey: string },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: mocks.close,
    isFullscreen: mocks.isFullscreen,
    isMaximized: mocks.isMaximized,
    minimize: mocks.minimize,
    onResized: mocks.onResized,
    setFullscreen: mocks.setFullscreen,
    toggleMaximize: mocks.toggleMaximize,
  }),
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: {
      expanded: mocks.leftSidebarExpanded,
      toggleExpanded: mocks.toggleExpanded,
    },
  }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.createNewNote,
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({ open: mocks.openNoteDialog }),
}));

vi.mock("~/sidebar/timeline/upcoming-meeting", () => ({
  useSidebarUpcomingMeetingStatus: () => mocks.upcomingMeetingStatus,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      currentTab: typeof mocks.currentTab;
      openNew: typeof mocks.openNew;
    }) => unknown,
  ) => selector({ currentTab: mocks.currentTab, openNew: mocks.openNew }),
}));

import { WindowsTitleBar } from "./windows-title-bar";

describe("WindowsTitleBar", () => {
  beforeEach(() => {
    mocks.close.mockClear();
    mocks.createNewNote.mockClear();
    mocks.isFullscreen.mockClear();
    mocks.isMaximized.mockClear();
    mocks.isMaximized.mockResolvedValue(false);
    mocks.minimize.mockClear();
    mocks.onResized.mockClear();
    mocks.openNew.mockClear();
    mocks.openNoteDialog.mockClear();
    mocks.openUrl.mockClear();
    mocks.setFullscreen.mockClear();
    mocks.toggleExpanded.mockClear();
    mocks.toggleMaximize.mockClear();
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mocks.platform = "windows";
    mocks.upcomingMeetingStatus = null;
    resetSidebarNotes();
  });

  afterEach(() => {
    cleanup();
    resetSidebarNotes();
  });

  it("renders the sidebar and application menus in the draggable title bar", async () => {
    render(<WindowsTitleBar showSidebarTimelineChrome />);

    const titleBar = screen.getByTestId("windows-title-bar");
    const sidebarToggle = screen.getByRole("button", { name: "Hide sidebar" });

    expect(titleBar.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(titleBar.className).toContain("bg-background");
    expect(titleBar.className).not.toContain("border-b");
    expect(
      sidebarToggle.compareDocumentPosition(
        screen.getByRole("menuitem", { name: "File" }),
      ),
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(screen.getByRole("menuitem", { name: "File" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Edit" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "View" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Help" })).toBeTruthy();

    await waitFor(() => expect(mocks.isMaximized).toHaveBeenCalledOnce());
  });

  it("connects the sidebar and native window controls", () => {
    render(<WindowsTitleBar showSidebarTimelineChrome />);

    fireEvent.click(screen.getByRole("button", { name: "Hide sidebar" }));
    fireEvent.click(screen.getByRole("button", { name: "Minimize" }));
    fireEvent.click(screen.getByRole("button", { name: "Maximize" }));
    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(mocks.toggleExpanded).toHaveBeenCalledOnce();
    expect(mocks.minimize).toHaveBeenCalledOnce();
    expect(mocks.toggleMaximize).toHaveBeenCalledOnce();
    expect(mocks.close).toHaveBeenCalledOnce();
  });

  it("shows note actions beside the sidebar toggle only while expanded", () => {
    const { rerender } = render(<WindowsTitleBar showSidebarTimelineChrome />);

    expect(
      screen
        .getAllByRole("button")
        .slice(0, 4)
        .map((button) => button.getAttribute("aria-label")),
    ).toEqual(["Hide sidebar", "Search", "New note", "Sort notes"]);

    fireEvent.click(screen.getByRole("button", { name: "Search" }));
    fireEvent.click(screen.getByRole("button", { name: "New note" }));
    expect(mocks.openNoteDialog).toHaveBeenCalledOnce();
    expect(mocks.createNewNote).toHaveBeenCalledOnce();

    mocks.leftSidebarExpanded = false;
    rerender(<WindowsTitleBar showSidebarTimelineChrome />);

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
    expect(screen.getByRole("button", { name: "Show sidebar" })).toBeTruthy();

    mocks.leftSidebarExpanded = true;
    rerender(<WindowsTitleBar showSidebarTimelineChrome />);
    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }
  });

  it("hides note actions outside timeline screens and restores them on return", () => {
    const { rerender } = render(
      <WindowsTitleBar showSidebarTimelineChrome={false} />,
    );

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
    expect(screen.getByRole("button", { name: "Hide sidebar" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "File" })).toBeTruthy();

    rerender(<WindowsTitleBar showSidebarTimelineChrome />);

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }

    rerender(<WindowsTitleBar showSidebarTimelineChrome={false} />);

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
  });

  it("changes timeline grouping from the title bar", () => {
    render(<WindowsTitleBar showSidebarTimelineChrome />);

    const filter = screen.getByRole("button", { name: "Sort notes" });
    fireEvent.pointerDown(filter);
    fireEvent.click(filter);
    const grouping = screen.getByRole("menuitem", { name: "Grouping, Date" });
    fireEvent.focus(grouping);
    fireEvent.keyDown(grouping, { key: "ArrowRight" });
    fireEvent.click(screen.getByRole("menuitem", { name: "Folder" }));

    expect(useSidebarNotes.getState().groupBy).toBe("folder");
  });

  it("keeps Linux note actions in the sidebar", () => {
    mocks.platform = "linux";
    render(<WindowsTitleBar showSidebarTimelineChrome />);

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
  });

  it("preserves the collapsed-sidebar upcoming meeting badge", () => {
    mocks.leftSidebarExpanded = false;
    mocks.upcomingMeetingStatus = { itemKey: "session-upcoming" };

    render(<WindowsTitleBar showSidebarTimelineChrome />);

    const toggle = screen.getByRole("button", { name: "Show sidebar" });
    expect(
      toggle.querySelector(
        "[data-testid='collapsed-sidebar-upcoming-meeting-badge']",
      ),
    ).not.toBeNull();
  });
});
