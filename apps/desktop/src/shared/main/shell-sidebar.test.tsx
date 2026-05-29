import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  sidebarTimelineEnabled: false,
  setExpanded: vi.fn(),
  setLocked: vi.fn(),
}));

const { setExpanded, setLocked } = hoisted;

let mockCurrentTab: {
  type: "settings" | "empty" | "onboarding" | "calendar";
} | null = { type: "empty" };
const mockLeftSidebar = {
  expanded: false,
  setExpanded,
  setLocked,
};

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mockLeftSidebar,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mockCurrentTab }) => unknown,
  ) => selector({ currentTab: mockCurrentTab }),
}));

vi.mock("~/sidebar", () => ({
  LeftSidebar: () => <div data-testid="left-sidebar" />,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => hoisted.sidebarTimelineEnabled,
}));

import { ClassicMainSidebar } from "~/main/shell-sidebar";

describe("ClassicMainSidebar", () => {
  beforeEach(() => {
    mockCurrentTab = { type: "empty" };
    mockLeftSidebar.expanded = false;
    hoisted.sidebarTimelineEnabled = false;
    setExpanded.mockClear();
    setLocked.mockClear();
  });

  it("forces custom-sidebar tabs open and restores the previous sidebar state", async () => {
    mockCurrentTab = { type: "settings" };

    const { rerender } = render(<ClassicMainSidebar />);

    expect(setExpanded).toHaveBeenCalledWith(true);
    expect(setLocked).toHaveBeenCalledWith(true);

    mockCurrentTab = { type: "empty" };

    rerender(<ClassicMainSidebar />);

    expect(setLocked).toHaveBeenLastCalledWith(false);
    expect(setExpanded).toHaveBeenLastCalledWith(false);
  });

  it("unlocks the custom sidebar when unmounted while active", () => {
    mockCurrentTab = { type: "calendar" };

    const { unmount } = render(<ClassicMainSidebar />);

    expect(setExpanded).toHaveBeenCalledWith(true);
    expect(setLocked).toHaveBeenCalledWith(true);

    unmount();

    expect(setLocked).toHaveBeenLastCalledWith(false);
    expect(setExpanded).toHaveBeenLastCalledWith(false);
  });

  it("renders the default timeline sidebar when the setting is enabled", () => {
    mockLeftSidebar.expanded = true;
    hoisted.sidebarTimelineEnabled = true;

    render(<ClassicMainSidebar />);

    expect(screen.getByTestId("left-sidebar")).toBeTruthy();
  });
});
