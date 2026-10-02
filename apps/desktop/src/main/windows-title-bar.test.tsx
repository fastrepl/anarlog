import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
  chatMode: "FloatingClosed",
  currentTab: { type: "empty" } as { id?: string; type: string },
  leftSidebarExpanded: true,
  openCurrent: vi.fn(),
  sendEvent: vi.fn(),
  platform: "windows",
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
    chat: { mode: mocks.chatMode, sendEvent: mocks.sendEvent },
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
  useSidebarUpcomingMeetingStatus: () => null,
}));

vi.mock("~/store/zustand/tabs", () => {
  const getState = () => ({
    canGoBack: false,
    currentTab: mocks.currentTab,
    goBack: vi.fn(),
    openCurrent: mocks.openCurrent,
    openNew: mocks.openNew,
    select: vi.fn(),
    tabs: [],
  });
  const useTabs = Object.assign(
    (selector: (state: ReturnType<typeof getState>) => unknown) =>
      selector(getState()),
    { getState },
  );

  return { uniqueIdfromTab: (tab: { type: string }) => tab.type, useTabs };
});

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
    mocks.openCurrent.mockClear();
    mocks.sendEvent.mockClear();
    mocks.chatMode = "FloatingClosed";
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mocks.platform = "windows";
  });

  afterEach(() => {
    cleanup();
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

  it("replaces the sidebar toggle with a back button on custom sidebar tabs", () => {
    mocks.currentTab = { type: "settings" };

    render(<WindowsTitleBar showSidebarTimelineChrome={false} />);

    expect(screen.queryByRole("button", { name: "Hide sidebar" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Show sidebar" })).toBeNull();
    expect(document.getElementById("title-bar-sidebar-actions")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.toggleExpanded).not.toHaveBeenCalled();
    expect(mocks.openCurrent).toHaveBeenCalledWith({ type: "empty" });
  });

  it("closes an open chat from the title bar back button first", () => {
    mocks.currentTab = { type: "calendar" };
    mocks.chatMode = "Floating";

    render(<WindowsTitleBar showSidebarTimelineChrome={false} />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it("shows note actions beside the sidebar toggle only while expanded", () => {
    const { rerender } = render(<WindowsTitleBar showSidebarTimelineChrome />);

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }

    fireEvent.click(screen.getByRole("button", { name: "Search" }));
    fireEvent.click(screen.getByRole("button", { name: "New note" }));
    fireEvent.click(screen.getByRole("button", { name: "Sort notes" }));
    expect(mocks.openNoteDialog).toHaveBeenCalledOnce();
    expect(mocks.createNewNote).toHaveBeenCalledOnce();

    mocks.leftSidebarExpanded = false;
    rerender(<WindowsTitleBar showSidebarTimelineChrome />);

    for (const name of ["Search", "New note", "Sort notes"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
  });
});
