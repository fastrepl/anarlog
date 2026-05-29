import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("~/main/useTabsShortcuts", () => ({
  useClassicMainTabsShortcuts: vi.fn(),
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
      showDevtool: false,
    },
  }),
}));

vi.mock("~/sidebar/toast", () => ({
  ToastArea: () => <div data-testid="toast-area" />,
}));

vi.mock("~/store/zustand/tabs", () => ({
  uniqueIdfromTab: vi.fn(() => "empty-slot"),
  useTabs: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      tabs: [{ active: true, pinned: false, slotId: "slot-1", type: "empty" }],
      currentTab: {
        active: true,
        pinned: false,
        slotId: "slot-1",
        type: "empty",
      },
    }),
  ),
}));

import { ClassicMainBody } from "~/main/body";

describe("ClassicMainBody", () => {
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
