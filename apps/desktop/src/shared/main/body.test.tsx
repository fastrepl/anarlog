import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openNew: vi.fn(),
  goBack: vi.fn(),
  goNext: vi.fn(),
  runEscapeShortcut: vi.fn(),
  isTauri: vi.fn(() => true),
  startDragging: vi.fn().mockResolvedValue(undefined),
  canGoBack: false,
  canGoNext: false,
  currentTab: {
    active: true,
    pinned: false,
    slotId: "slot-1",
    type: "empty",
  },
  sidebarTimelineEnabled: false,
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: mocks.isTauri,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    startDragging: mocks.startDragging,
  }),
}));

vi.mock("~/main/useTabsShortcuts", () => ({
  useClassicMainTabsShortcuts: vi.fn(() => ({
    runEscapeShortcut: mocks.runEscapeShortcut,
  })),
}));

vi.mock("~/main/tab-content", () => ({
  ClassicMainTabContent: ({ tab }: { tab: { type: string } }) => (
    <div data-testid="main-tab-content">{tab.type}</div>
  ),
}));

vi.mock("~/main/top-meeting-timeline", () => ({
  TopMeetingTimeline: () => <div data-testid="top-meeting-timeline" />,
}));

vi.mock("~/main/shell-sidebar", () => ({
  ClassicMainSidebar: () => <div data-testid="main-sidebar" />,
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: {
      expanded: true,
    },
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.sidebarTimelineEnabled,
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
    mocks.openNew.mockClear();
    mocks.goBack.mockClear();
    mocks.goNext.mockClear();
    mocks.runEscapeShortcut.mockClear();
    mocks.isTauri.mockReturnValue(true);
    mocks.startDragging.mockClear();
    mocks.canGoBack = false;
    mocks.canGoNext = false;
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "empty",
    };
    mocks.sidebarTimelineEnabled = false;
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the shell and current tab content", () => {
    render(<ClassicMainBody />);

    const timeline = screen.getByTestId("top-meeting-timeline");
    const timelineRow = timeline.parentElement?.parentElement;
    const topArea = timelineRow?.parentElement;

    expect(timeline).toBeTruthy();
    expect(timelineRow?.className).toContain("pl-[76px]");
    expect(timelineRow?.className).toContain("pt-1");
    expect(timelineRow?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(timeline.parentElement?.className).toContain("flex-1");
    expect(topArea?.className).toContain("h-12");
    expect(topArea?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(screen.getByTestId("main-sidebar")).toBeTruthy();
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "empty",
    );
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

    expect(screen.queryByTestId("top-meeting-timeline")).toBeNull();
    expect(screen.queryByTestId("toast-area")).toBeNull();
    expect(firstBodyChild?.className).toContain(
      "flex min-h-0 min-w-0 flex-1 gap-1",
    );
    expect(firstBodyChild?.hasAttribute("data-tauri-drag-region")).toBe(false);
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "onboarding",
    );
  });

  it("hides the top timeline when the sidebar timeline is enabled", () => {
    mocks.sidebarTimelineEnabled = true;

    render(<ClassicMainBody />);

    expect(screen.queryByTestId("top-meeting-timeline")).toBeNull();
    expect(screen.queryByTestId("toast-area")).toBeNull();
    const sidebar = screen.getByTestId("main-sidebar");
    const backButton = screen.getByRole("button", { name: "Go back" });
    const topArea = backButton.parentElement?.parentElement?.parentElement;

    expect(sidebar).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open calendar" })).toBeNull();
    expect(backButton.hasAttribute("disabled")).toBe(true);
    expect(
      screen
        .getByRole("button", { name: "Go forward" })
        .hasAttribute("disabled"),
    ).toBe(true);
    expect(topArea?.className).toContain("h-12");
    expect(topArea?.className).toContain("absolute");
    expect(backButton.parentElement?.parentElement?.className).toContain(
      "pt-[9px]",
    );
    expect(sidebar.parentElement?.className).toContain("flex min-h-0");
    expect(sidebar.parentElement?.className).not.toContain("pt-12");
  });

  it("hides timeline chrome for changelog tabs without collapsing the sidebar state", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "changelog",
    };

    const { container } = render(<ClassicMainBody />);
    const body = container.firstElementChild;
    const topArea = body?.firstElementChild;

    expect(screen.queryByTestId("top-meeting-timeline")).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(topArea?.className).toContain("h-10");
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "changelog",
    );
  });

  it("keeps sidebar timeline chrome for changelog tabs in sidebar timeline mode", () => {
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "changelog",
    };
    mocks.sidebarTimelineEnabled = true;

    render(<ClassicMainBody />);

    const backButton = screen.getByRole("button", { name: "Go back" });
    const topArea = backButton.parentElement?.parentElement?.parentElement;

    expect(screen.queryByTestId("top-meeting-timeline")).toBeNull();
    expect(backButton.hasAttribute("disabled")).toBe(true);
    expect(topArea?.className).toContain("h-12");
    expect(topArea?.className).toContain("absolute");
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "changelog",
    );
  });

  it("navigates history from the sidebar timeline chrome", () => {
    mocks.sidebarTimelineEnabled = true;
    mocks.canGoBack = true;
    mocks.canGoNext = true;

    render(<ClassicMainBody />);

    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    fireEvent.click(screen.getByRole("button", { name: "Go forward" }));

    expect(mocks.openNew).not.toHaveBeenCalled();
    expect(mocks.goBack).toHaveBeenCalledTimes(1);
    expect(mocks.goNext).toHaveBeenCalledTimes(1);
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

      expect(screen.queryByTestId("top-meeting-timeline")).toBeNull();
      expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
      expect(backButton.hasAttribute("disabled")).toBe(false);
      expect(topArea?.className).toContain("h-12");
      expect(topArea?.className).toContain("absolute");
      expect(mocks.goBack).not.toHaveBeenCalled();
      expect(mocks.runEscapeShortcut).toHaveBeenCalledTimes(1);
    },
  );

  it("starts window dragging from the top 48px of the main area in sidebar timeline mode", () => {
    mocks.sidebarTimelineEnabled = true;

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

  it("does not start window dragging below the main area drag strip", () => {
    mocks.sidebarTimelineEnabled = true;

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

  it("does not add main area dragging when the top timeline owns the titlebar", () => {
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
    expect(view.getByTestId("top-meeting-timeline")).toBeTruthy();
    expect(view.queryByTestId("main-tab-content")).toBeNull();
  });
});
