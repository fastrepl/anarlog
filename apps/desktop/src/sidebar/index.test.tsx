import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("~/sidebar/timeline", () => ({
  TimelineView: ({
    showOpenCalendarButton = true,
    topChromeInset = false,
  }: {
    showOpenCalendarButton?: boolean;
    topChromeInset?: boolean;
  }) => (
    <div
      data-testid="timeline-view"
      data-show-open-calendar-button={String(showOpenCalendarButton)}
      data-top-chrome-inset={String(topChromeInset)}
    />
  ),
}));

vi.mock("~/sidebar/toast", () => ({
  ToastArea: () => <div data-testid="toast-area" />,
}));

vi.mock("~/sidebar/calendar", () => ({
  CalendarNav: () => <div data-testid="calendar-nav" />,
}));

vi.mock("~/sidebar/contacts", () => ({
  ContactsNav: () => <div data-testid="contacts-nav" />,
}));

vi.mock("~/sidebar/settings", () => ({
  SettingsNav: () => <div data-testid="settings-nav" />,
}));

vi.mock("~/sidebar/templates", () => ({
  TemplatesNav: () => <div data-testid="templates-nav" />,
}));

import { LeftSidebar } from "./index";

describe("LeftSidebar", () => {
  beforeEach(() => {
    mocks.currentTab = { type: "empty" };
  });

  afterEach(() => {
    cleanup();
  });

  it("uses the timeline layout without a duplicate sidebar top offset", () => {
    const { container } = render(<LeftSidebar />);

    expect(screen.getByTestId("timeline-view")).toBeTruthy();
    expect(
      screen
        .getByTestId("timeline-view")
        .getAttribute("data-show-open-calendar-button"),
    ).toBe("true");
    expect(
      screen.getByTestId("timeline-view").getAttribute("data-top-chrome-inset"),
    ).toBe("true");
    expect(container.firstElementChild?.className).toContain("pt-0");
    expect(container.firstElementChild?.className).not.toContain("pr-1");
  });

  it("renders timeline header as normal sidebar content", () => {
    render(
      <LeftSidebar timelineHeader={<div data-testid="timeline-header" />} />,
    );

    expect(
      screen
        .getByTestId("timeline-header")
        .parentElement?.contains(screen.getByTestId("timeline-view")),
    ).toBe(true);
    expect(
      screen.getByTestId("timeline-view").getAttribute("data-top-chrome-inset"),
    ).toBe("false");
  });

  it.each([
    ["settings", "settings-nav"],
    ["calendar", "calendar-nav"],
    ["contacts", "contacts-nav"],
    ["templates", "templates-nav"],
  ])("keeps %s below the window chrome", (type, testId) => {
    mocks.currentTab = { type };

    const { container } = render(<LeftSidebar />);
    const classList = container.firstElementChild?.className.split(" ") ?? [];

    expect(screen.getByTestId(testId)).toBeTruthy();
    expect(classList).toContain("pt-11");
    expect(classList).toContain("pr-1");
    expect(classList).not.toContain("pt-0");
  });
});
