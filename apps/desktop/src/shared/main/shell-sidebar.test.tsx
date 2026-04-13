import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  setExpanded: vi.fn(),
  setLocked: vi.fn(),
}));

const { setExpanded, setLocked } = hoisted;

let mockQuery = "";
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

vi.mock("~/search/contexts/ui", () => ({
  useSearch: () => ({
    query: mockQuery,
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

import { ClassicMainSidebar } from "~/main/shell-sidebar";

describe("ClassicMainSidebar", () => {
  beforeEach(() => {
    mockQuery = "";
    mockCurrentTab = { type: "empty" };
    mockLeftSidebar.expanded = false;
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

  it("expands the sidebar when search starts from an empty query", () => {
    const { rerender } = render(<ClassicMainSidebar />);

    setExpanded.mockClear();

    mockQuery = "meeting";

    rerender(<ClassicMainSidebar />);

    expect(setExpanded).toHaveBeenCalledWith(true);
  });
});
