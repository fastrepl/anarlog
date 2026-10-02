import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  canGoBack: false,
  chatMode: "FloatingClosed",
  currentTab: { type: "settings" } as {
    returnToSlotId?: string;
    returnToTabId?: string;
    slotId?: string;
    type: string;
  } | null,
  goBack: vi.fn(),
  openCurrent: vi.fn(),
  platform: null as string | null,
  select: vi.fn(),
  sendEvent: vi.fn(),
  tabs: [] as {
    returnToSlotId?: string;
    returnToTabId?: string;
    slotId?: string;
    type: string;
  }[],
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => {
    if (mocks.platform === null) {
      throw new Error("not in tauri");
    }
    return mocks.platform;
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

vi.mock("~/store/zustand/tabs", () => {
  const getState = () => ({
    canGoBack: mocks.canGoBack,
    currentTab: mocks.currentTab,
    goBack: mocks.goBack,
    openCurrent: mocks.openCurrent,
    select: mocks.select,
    tabs: mocks.tabs,
  });
  const useTabs = Object.assign(
    (selector: (state: unknown) => unknown) => selector(getState()),
    { getState },
  );

  return {
    uniqueIdfromTab: (tab: { type: string }) => tab.type,
    useTabs,
  };
});

import {
  CustomSidebarHeader,
  TITLE_BAR_SIDEBAR_ACTIONS_SLOT_ID,
} from "./custom-sidebar-header";

describe("CustomSidebarHeader", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.canGoBack = false;
    mocks.chatMode = "FloatingClosed";
    mocks.currentTab = { type: "settings" };
    mocks.goBack.mockClear();
    mocks.openCurrent.mockClear();
    mocks.platform = null;
    mocks.select.mockClear();
    mocks.sendEvent.mockClear();
    mocks.tabs = [];
  });

  it("opens home from the back button", () => {
    render(<CustomSidebarHeader />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.openCurrent).toHaveBeenCalledWith({ type: "empty" });
  });

  it("selects an existing home tab from the back button", () => {
    const homeTab = { type: "empty" };
    mocks.tabs = [homeTab];

    render(<CustomSidebarHeader />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.select).toHaveBeenCalledWith(homeTab);
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it("returns a workspace opened from settings back to settings", () => {
    const settingsTab = { slotId: "slot-settings", type: "settings" };
    mocks.currentTab = {
      returnToSlotId: "slot-settings",
      returnToTabId: "settings",
      slotId: "slot-folders",
      type: "folders",
    };
    mocks.tabs = [settingsTab, mocks.currentTab, { type: "empty" }];

    render(<CustomSidebarHeader />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.select).toHaveBeenCalledWith(settingsTab);
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it.each(["FloatingOpen", "RightPanelOpen"])(
    "closes %s chat before opening home",
    (chatMode) => {
      mocks.chatMode = chatMode;

      render(<CustomSidebarHeader />);

      fireEvent.click(screen.getByRole("button", { name: "Go home" }));

      expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
      expect(mocks.openCurrent).not.toHaveBeenCalled();
    },
  );

  it("opens home directly from Automations without collapsing its chat", () => {
    mocks.chatMode = "RightPanelOpen";
    mocks.currentTab = { type: "automations" };

    render(<CustomSidebarHeader />);

    fireEvent.click(screen.getByRole("button", { name: "Go home" }));

    expect(mocks.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(mocks.sendEvent).not.toHaveBeenCalled();
  });

  it("portals header actions next to the title bar back button on windows", () => {
    mocks.platform = "windows";
    const slot = document.createElement("div");
    slot.id = TITLE_BAR_SIDEBAR_ACTIONS_SLOT_ID;
    document.body.appendChild(slot);

    try {
      const { container } = render(
        <CustomSidebarHeader>
          <button type="button">New folder</button>
        </CustomSidebarHeader>,
      );

      expect(container.innerHTML).toBe("");
      expect(
        slot.contains(screen.getByRole("button", { name: "New folder" })),
      ).toBe(true);
    } finally {
      slot.remove();
    }
  });
});
