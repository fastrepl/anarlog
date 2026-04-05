import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./tab-chrome", () => ({
  MainTabChrome: () => <div data-testid="main-tab-chrome" />,
}));

vi.mock("./tab-content", () => ({
  MainTabContent: ({ tab }: { tab: { type: string } }) => (
    <div data-testid="main-tab-content">{tab.type}</div>
  ),
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

import { Body } from "./body";

describe("Body", () => {
  it("renders the extracted tab chrome and current tab content", () => {
    render(<Body />);

    expect(screen.getByTestId("main-tab-chrome")).toBeTruthy();
    expect(screen.getByTestId("main-tab-content").textContent).toContain(
      "empty",
    );
  });

  it("returns nothing when there is no current tab", async () => {
    const { useTabs } = await import("~/store/zustand/tabs");

    vi.mocked(useTabs).mockImplementationOnce(((
      selector: (state: unknown) => unknown,
    ) =>
      selector({
        tabs: [],
        currentTab: null,
      })) as typeof useTabs);

    const { container } = render(<Body />);

    expect(container.firstChild).toBeNull();
  });
});
