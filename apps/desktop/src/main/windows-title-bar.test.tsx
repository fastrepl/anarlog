import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  close: vi.fn().mockResolvedValue(undefined),
  createNewNote: vi.fn(),
  isFullscreen: vi.fn().mockResolvedValue(false),
  isMaximized: vi.fn().mockResolvedValue(false),
  minimize: vi.fn().mockResolvedValue(undefined),
  onResized: vi.fn().mockResolvedValue(vi.fn()),
  openNew: vi.fn(),
  openUrl: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  setFullscreen: vi.fn().mockResolvedValue(undefined),
  toggleExpanded: vi.fn(),
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
  currentTab: { type: "empty" } as { id?: string; type: string },
  leftSidebarExpanded: true,
  upcomingMeetingStatus: null as null | { itemKey: string },
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
    mocks.openUrl.mockClear();
    mocks.setFullscreen.mockClear();
    mocks.toggleExpanded.mockClear();
    mocks.toggleMaximize.mockClear();
    mocks.currentTab = { type: "empty" };
    mocks.leftSidebarExpanded = true;
    mocks.upcomingMeetingStatus = null;
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the sidebar and application menus in the draggable title bar", async () => {
    render(<WindowsTitleBar />);

    const titleBar = screen.getByTestId("windows-title-bar");

    expect(titleBar.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(screen.getByRole("button", { name: "Hide sidebar" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "File" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Edit" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "View" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Help" })).toBeTruthy();

    await waitFor(() => expect(mocks.isMaximized).toHaveBeenCalledOnce());
  });

  it("connects the sidebar and native window controls", () => {
    render(<WindowsTitleBar />);

    fireEvent.click(screen.getByRole("button", { name: "Hide sidebar" }));
    fireEvent.click(screen.getByRole("button", { name: "Minimize" }));
    fireEvent.click(screen.getByRole("button", { name: "Maximize" }));
    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    expect(mocks.toggleExpanded).toHaveBeenCalledOnce();
    expect(mocks.minimize).toHaveBeenCalledOnce();
    expect(mocks.toggleMaximize).toHaveBeenCalledOnce();
    expect(mocks.close).toHaveBeenCalledOnce();
  });

  it("preserves the collapsed-sidebar upcoming meeting badge", () => {
    mocks.leftSidebarExpanded = false;
    mocks.upcomingMeetingStatus = { itemKey: "session-upcoming" };

    render(<WindowsTitleBar />);

    const toggle = screen.getByRole("button", { name: "Show sidebar" });
    expect(
      toggle.querySelector(
        "[data-testid='collapsed-sidebar-upcoming-meeting-badge']",
      ),
    ).not.toBeNull();
  });
});
