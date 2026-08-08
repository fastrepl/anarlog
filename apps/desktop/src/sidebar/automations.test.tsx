import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  automations: [
    {
      id: "automation-1",
      ownerUserId: "user-1",
      title: "Share weekly recap",
      createdAt: "2026-08-03T10:00:00.000Z",
      updatedAt: "2026-08-03T10:00:00.000Z",
    },
    {
      id: "automation-2",
      ownerUserId: "user-1",
      title: "Update project notes",
      createdAt: "2026-08-02T10:00:00.000Z",
      updatedAt: "2026-08-02T10:00:00.000Z",
    },
  ],
  selectedAutomationId: "automation-1" as string | undefined,
  selectChat: vi.fn(),
  startNewChat: vi.fn(),
}));

vi.mock("~/chat/store/queries", () => ({
  useChatGroups: () => mocks.automations,
}));

vi.mock("~/chat/state/chat-context", () => ({
  useChatContext: (selector: (state: unknown) => unknown) =>
    selector({
      chatByScope: {
        automations: { groupId: mocks.selectedAutomationId },
      },
      selectChat: mocks.selectChat,
      startNewChat: mocks.startNewChat,
    }),
}));

vi.mock("~/sidebar/custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ children }: { children?: React.ReactNode }) => (
    <header>{children}</header>
  ),
}));

import { AutomationsNav } from "./automations";

describe("AutomationsNav", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.automations = [
      {
        id: "automation-1",
        ownerUserId: "user-1",
        title: "Share weekly recap",
        createdAt: "2026-08-03T10:00:00.000Z",
        updatedAt: "2026-08-03T10:00:00.000Z",
      },
      {
        id: "automation-2",
        ownerUserId: "user-1",
        title: "Update project notes",
        createdAt: "2026-08-02T10:00:00.000Z",
        updatedAt: "2026-08-02T10:00:00.000Z",
      },
    ];
    mocks.selectedAutomationId = "automation-1";
    mocks.selectChat.mockClear();
    mocks.startNewChat.mockClear();
  });

  it("lists past automations and opens the selected automation conversation", () => {
    render(<AutomationsNav />);

    expect(screen.getByText("Share weekly recap")).toBeTruthy();
    expect(screen.getByText("Update project notes")).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: /Share weekly recap/ })
        .getAttribute("aria-current"),
    ).toBe("page");

    fireEvent.click(
      screen.getByRole("button", { name: /Update project notes/ }),
    );

    expect(mocks.selectChat).toHaveBeenCalledWith(
      "automations",
      "automation-2",
    );
  });

  it("filters the automation list and clears the search", () => {
    render(<AutomationsNav />);

    fireEvent.change(screen.getByPlaceholderText("Search automations..."), {
      target: { value: "project" },
    });

    expect(screen.queryByText("Share weekly recap")).toBeNull();
    expect(screen.getByText("Update project notes")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));

    expect(screen.getByText("Share weekly recap")).toBeTruthy();
  });

  it("starts a new automation conversation from the sidebar", () => {
    render(<AutomationsNav />);

    fireEvent.click(screen.getByRole("button", { name: "New automation" }));

    expect(mocks.startNewChat).toHaveBeenCalledWith("automations");
  });

  it("shows an empty state when no automations exist", () => {
    mocks.automations = [];

    render(<AutomationsNav />);

    expect(screen.getByText("No automations yet")).toBeTruthy();
  });
});
