import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  canGoBack: false,
  canGoNext: false,
  chatMode: "FloatingClosed",
  currentTab: { type: "settings" } as { type: string } | null,
  goBack: vi.fn(),
  goNext: vi.fn(),
  openCurrent: vi.fn(),
  select: vi.fn(),
  sendEvent: vi.fn(),
  tabs: [] as { type: string }[],
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      currentTab: mocks.currentTab,
      canGoBack: mocks.canGoBack,
      canGoNext: mocks.canGoNext,
      goBack: mocks.goBack,
      goNext: mocks.goNext,
      openCurrent: mocks.openCurrent,
      select: mocks.select,
      tabs: mocks.tabs,
    }),
}));

import { CustomSidebarHeader } from "./custom-sidebar-header";

describe("CustomSidebarHeader", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.canGoBack = false;
    mocks.canGoNext = false;
    mocks.chatMode = "FloatingClosed";
    mocks.currentTab = { type: "settings" };
    mocks.goBack.mockClear();
    mocks.goNext.mockClear();
    mocks.openCurrent.mockClear();
    mocks.select.mockClear();
    mocks.sendEvent.mockClear();
    mocks.tabs = [];
  });

  it("opens home from the back button", () => {
    render(<CustomSidebarHeader title="Settings" />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.openCurrent).toHaveBeenCalledWith({ type: "empty" });
  });

  it("selects an existing home tab from the back button", () => {
    const homeTab = { type: "empty" };
    mocks.tabs = [homeTab];

    render(<CustomSidebarHeader title="Calendar" />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.select).toHaveBeenCalledWith(homeTab);
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it("closes floating chat before opening home", () => {
    mocks.chatMode = "FloatingOpen";

    render(<CustomSidebarHeader title="Contacts" />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it("renders history controls when requested", () => {
    mocks.canGoBack = true;
    mocks.canGoNext = true;

    render(<CustomSidebarHeader title="Contacts" showHistoryControls />);

    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    fireEvent.click(screen.getByRole("button", { name: "Go forward" }));

    expect(mocks.goBack).toHaveBeenCalledTimes(1);
    expect(mocks.goNext).toHaveBeenCalledTimes(1);
  });

  it("hides history controls by default", () => {
    render(<CustomSidebarHeader title="Settings" />);

    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
  });
});
