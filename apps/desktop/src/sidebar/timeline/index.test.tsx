import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  anchorNode: null as HTMLDivElement | null,
  openNew: vi.fn(),
  registerAnchor: vi.fn(),
  useAutoScrollToAnchor: vi.fn(),
  invalidateResource: vi.fn(),
  clearSelection: vi.fn(),
  currentTab: { type: "empty" } as
    | { type: "empty" }
    | { type: "sessions"; id: string },
  deleteSession: vi.fn(),
  configValue: undefined as string | undefined,
  currentTimeMs: undefined as number | undefined,
  isAnchorVisible: true,
  isScrolledPastAnchor: false,
  isIgnored: vi.fn(() => false),
  liveSessionId: null as string | null,
  liveStatus: "inactive" as "inactive" | "active" | "finalizing",
  selectAll: vi.fn(),
  smartCurrentTimeMs: undefined as number | undefined,
  timelineSelectionAnchorId: null as string | null,
  timelineSelectionSelectedIds: [] as string[],
  timelineEventsTable: {} as Record<string, Record<string, unknown>>,
  timelineSessionsTable: {} as Record<string, Record<string, unknown>>,
}));

const lingui = vi.hoisted(() => {
  const t = (
    input:
      | TemplateStringsArray
      | { message?: string; values?: Record<string, unknown> }
      | string,
    ...values: unknown[]
  ) => {
    if (Array.isArray(input)) {
      const message = input.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );

      return message === "Now" ? "Localized now" : message;
    }

    if (typeof input === "string") {
      return input;
    }

    if ("message" in input) {
      if (input.message === "Now") {
        return "Localized now";
      }

      return (input.message ?? "").replace(
        /\{(\w+)\}/g,
        (_match: string, key: string) =>
          String(input.values?.[key] ?? `{${key}}`),
      );
    }

    return "";
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
  }: {
    children?: ReactNode;
    id?: string;
    message?: string;
  }) => <>{children ?? message ?? id}</>,
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@lingui/react", () => ({
  Trans: ({
    children,
    id,
    message,
  }: {
    children?: ReactNode;
    id?: string;
    message?: string;
  }) => <>{children ?? message ?? id}</>,
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.configValue,
}));

vi.mock("~/calendar/queries", () => ({
  useTimelineTables: () => ({
    timelineEventsTable: mocks.timelineEventsTable,
    timelineSessionsTable: mocks.timelineSessionsTable,
  }),
}));

vi.mock("~/session/hooks/useDeleteSession", () => ({
  useDeleteSession: () => mocks.deleteSession,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    isIgnored: mocks.isIgnored,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      currentTab: mocks.currentTab,
      invalidateResource: mocks.invalidateResource,
      openNew: mocks.openNew,
    }),
}));

vi.mock("~/store/zustand/timeline-selection", () => ({
  useTimelineSelection: (selector: (state: unknown) => unknown) =>
    selector({
      anchorId: mocks.timelineSelectionAnchorId,
      clear: mocks.clearSelection,
      selectAll: mocks.selectAll,
      selectedIds: mocks.timelineSelectionSelectedIds,
    }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: {
        sessionId: string | null;
        status: "inactive" | "active" | "finalizing";
      };
    }) => unknown,
  ) =>
    selector({
      live: {
        sessionId: mocks.liveSessionId,
        status: mocks.liveStatus,
      },
    }),
}));

vi.mock("./anchor", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    useAnchor: () => ({
      anchorNode: mocks.anchorNode,
      containerRef: React.useRef<HTMLDivElement>(null),
      isAnchorVisible: mocks.isAnchorVisible,
      isScrolledPastAnchor: mocks.isScrolledPastAnchor,
      registerAnchor: mocks.registerAnchor,
      scrollToAnchor: vi.fn(),
    }),
    useAutoScrollToAnchor: mocks.useAutoScrollToAnchor,
  };
});

vi.mock("./item", () => ({
  TimelineItemComponent: ({
    isUpcoming,
    item,
    itemNodeRef,
    upcomingLabel,
    upcomingProgress,
  }: {
    isUpcoming?: boolean;
    item: { id: string };
    itemNodeRef?: (node: HTMLDivElement | null) => void;
    upcomingLabel?: string;
    upcomingProgress?: number;
  }) => (
    <div
      ref={itemNodeRef}
      data-testid={`timeline-item-${item.id}`}
      data-upcoming={isUpcoming ? "true" : undefined}
      data-upcoming-label={upcomingLabel}
      data-upcoming-progress={upcomingProgress}
    />
  ),
}));

vi.mock("./realtime", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    CurrentTimeIndicator: React.forwardRef<HTMLDivElement>(
      function CurrentTimeIndicator(_props, ref) {
        return <div ref={ref} data-testid="current-time-indicator" />;
      },
    ),
    useCurrentTimeMs: () => mocks.currentTimeMs ?? Date.now(),
    useSmartCurrentTime: () => mocks.smartCurrentTimeMs ?? Date.now(),
  };
});

import { TimelineView } from ".";

describe("TimelineView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.anchorNode = null;
    mocks.configValue = undefined;
    mocks.currentTimeMs = undefined;
    mocks.isAnchorVisible = true;
    mocks.isScrolledPastAnchor = false;
    mocks.isIgnored.mockReturnValue(false);
    mocks.liveSessionId = null;
    mocks.liveStatus = "inactive";
    mocks.currentTab = { type: "empty" };
    mocks.selectAll.mockClear();
    mocks.smartCurrentTimeMs = undefined;
    mocks.timelineSelectionAnchorId = null;
    mocks.timelineSelectionSelectedIds = [];
    mocks.timelineEventsTable = {};
    mocks.timelineSessionsTable = {};
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("does not render sidebar action tabs inside the timeline chrome", () => {
    render(<TimelineView topChromeInset />);

    expect(screen.queryByRole("button", { name: "New note" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Search" })).toBeNull();
    expect(getSidebarActionTabsOrNull()).toBeNull();
  });

  it("shows the open calendar chip in top chrome without action tabs", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const calendarButton = screen.getByRole("button", {
      name: "Open calendar",
    });

    expect(getSidebarActionTabsOrNull()).toBeNull();
    expect(calendarButton.className).toContain("rounded-full");
    expect(
      container.querySelector("[data-sidebar-timeline-top-chip-stack]")
        ?.className,
    ).toContain("top-4");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-14");
    expect(queryTopOccluder(container)?.className).toContain("h-12");

    fireEvent.click(calendarButton);

    expect(mocks.openNew).toHaveBeenCalledWith({ type: "calendar" });
  });

  it("keeps the open calendar spacer stable when leaving the top edge", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");
    const topSpacer = container.querySelector(
      "[data-sidebar-timeline-top-spacer]",
    );

    expect(scroller).toBeInstanceOf(HTMLDivElement);
    expect(topSpacer?.className).toContain("h-14");

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(screen.queryByRole("button", { name: "Open calendar" })).toBeNull();
    expect(topSpacer?.className).toContain("h-14");
    expect(queryTopFade(container)).toBeNull();
    expect(queryTopOccluder(container)?.className).toContain("bg-background");
  });

  it("routes wheel gestures from the open calendar chip into the timeline scroller", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");
    const calendarButton = screen.getByRole("button", {
      name: "Open calendar",
    });

    expect(scroller).toBeInstanceOf(HTMLDivElement);

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });

    fireEvent.wheel(calendarButton, { deltaY: 80 });

    expect(scroller!.scrollTop).toBe(80);
  });

  it("keeps the first bucket below the sidebar action chrome", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      past: {
        title: "Demo Session Kickoff",
        created_at: "2024-01-01T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);

    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-12");
    expect(
      container.querySelector("[data-sidebar-timeline-bucket-header]")
        ?.className,
    ).toContain("top-12");
    expect(queryTopOccluder(container)?.className).toContain("h-12");
  });

  it("pins bucket headers to the sidebar chrome while scrolled", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      tomorrow: {
        title: "Founder sync",
        created_at: "2024-01-16T10:00:00.000Z",
      },
      today: {
        title: "Design sync",
        created_at: "2024-01-15T17:30:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");
    const header = container.querySelector(
      "[data-sidebar-timeline-bucket-header]",
    );

    expect(scroller).toBeInstanceOf(HTMLDivElement);
    expect(header?.className).toContain("top-12");

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(header?.className).toContain("top-12");
    expect(header?.className).toContain("z-20");
    expect(header?.className).toContain("bg-background");
    expect(header?.className).not.toContain("backdrop-blur");
    expect(container.querySelector("[class*='backdrop-blur']")).toBeNull();
    expect(queryTopOccluder(container)?.className).toContain("z-10");
  });

  it("shows the open calendar chip without top chrome", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView />);
    const calendarButton = screen.getByRole("button", {
      name: "Open calendar",
    });

    expect(getSidebarActionTabsOrNull()).toBeNull();
    expect(
      container.querySelector("[data-sidebar-timeline-top-chip-stack]")
        ?.className,
    ).toContain("top-2");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-8");

    fireEvent.click(calendarButton);

    expect(mocks.openNew).toHaveBeenCalledWith({ type: "calendar" });
  });

  it("keeps overlapping header chips inside the visible timeline", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChipsOverlapHeader />);

    expect(
      container.querySelector("[data-sidebar-timeline-root]")?.className,
    ).not.toContain("-mt-3");
    expect(
      container.querySelector("[data-sidebar-timeline-top-chip-stack]")
        ?.className,
    ).toContain("top-1");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-9");
  });

  it("selects all visible notes with Cmd+A after a sidebar note selection", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T09:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.currentTab = { type: "sessions", id: "selected-note" };
    mocks.timelineSelectionAnchorId = "session-selected-note";
    mocks.timelineEventsTable = {
      event: {
        title: "Calendar hold",
        started_at: "2024-01-15T13:00:00.000Z",
        ended_at: "2024-01-15T13:30:00.000Z",
        tracking_id_event: "event-hold",
        has_recurrence_rules: false,
      },
    };
    mocks.timelineSessionsTable = {
      "selected-note": {
        title: "Selected note",
        created_at: "2024-01-15T12:00:00.000Z",
      },
      "other-note": {
        title: "Other note",
        created_at: "2024-01-15T11:00:00.000Z",
      },
    };

    render(<TimelineView />);

    fireEvent.keyDown(window, { key: "a", metaKey: true });

    expect(mocks.selectAll).toHaveBeenCalledWith([
      "session-selected-note",
      "session-other-note",
    ]);
  });

  it("does not select sidebar notes when Cmd+A starts in the editor", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T09:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.currentTab = { type: "sessions", id: "selected-note" };
    mocks.timelineSelectionAnchorId = "session-selected-note";
    mocks.timelineSessionsTable = {
      "selected-note": {
        title: "Selected note",
        created_at: "2024-01-15T12:00:00.000Z",
      },
      "other-note": {
        title: "Other note",
        created_at: "2024-01-15T11:00:00.000Z",
      },
    };

    render(<TimelineView />);

    const editor = document.createElement("div");
    editor.className = "ProseMirror";
    editor.contentEditable = "true";
    editor.tabIndex = 0;
    document.body.appendChild(editor);
    editor.focus();

    fireEvent.keyDown(editor, { key: "a", metaKey: true });

    expect(mocks.selectAll).not.toHaveBeenCalled();

    editor.remove();
  });

  it("does not show a top chrome fade while scrolled without timeline action tabs", () => {
    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");

    expect(scroller).toBeInstanceOf(HTMLDivElement);
    expect(getSidebarActionTabsOrNull()).toBeNull();
    expect(queryTopFade(container)).toBeNull();
    expect(queryTopOccluder(container)?.className).toContain("h-12");

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(getSidebarActionTabsOrNull()).toBeNull();
    expect(queryTopFade(container)).toBeNull();
    expect(queryTopOccluder(container)?.className).toContain("h-12");
  });

  it("does not show a top scroll fade when there are no hidden future notes", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.timelineSessionsTable = {
      today: {
        title: "Design sync",
        created_at: "2024-01-15T11:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector(
      "[data-sidebar-timeline-scroll]",
    ) as HTMLDivElement | null;

    expect(scroller).toBeInstanceOf(HTMLDivElement);

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(scroller!.style.maskImage).toBe("");
    expect(queryBottomFade(container)).toBeTruthy();
  });

  it("does not show a top scroll fade when future notes are hidden above a sticky header", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.timelineSessionsTable = {
      today: {
        title: "Design sync",
        created_at: "2024-01-15T11:00:00.000Z",
      },
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector(
      "[data-sidebar-timeline-scroll]",
    ) as HTMLDivElement | null;

    expect(scroller).toBeInstanceOf(HTMLDivElement);

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(screen.getByText("Today")).toBeTruthy();
    expect(queryTopFade(container)).toBeNull();
    expect(queryTopOccluder(container)?.className).toContain("h-12");
    expect(scroller!.style.maskImage).toBe("");
    expect(queryBottomFade(container)).toBeTruthy();
  });

  it("drops the bottom scroll fade at the bottom edge", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.timelineSessionsTable = {
      today: {
        title: "Design sync",
        created_at: "2024-01-15T11:00:00.000Z",
      },
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector(
      "[data-sidebar-timeline-scroll]",
    ) as HTMLDivElement | null;

    expect(scroller).toBeInstanceOf(HTMLDivElement);

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 1000;
    fireEvent.scroll(scroller!);

    expect(scroller!.style.maskImage).toBe("");
    expect(queryBottomFade(container)).toBeNull();
  });

  it("keeps the bottom now chip above the scroll fade", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.isAnchorVisible = false;
    mocks.isScrolledPastAnchor = false;
    mocks.timelineSessionsTable = {
      later: {
        title: "Design sync",
        created_at: "2024-01-16T11:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView />);
    const scroller = container.querySelector(
      "[data-sidebar-timeline-scroll]",
    ) as HTMLDivElement;
    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller.scrollTop = 120;
    fireEvent.scroll(scroller);
    const nowChip = screen.getByRole("button", { name: "Go back to now" });

    expect(queryBottomFade(container)?.className).toContain("z-30");
    expect(nowChip.className).toContain("z-40");
  });

  it("shows an imminent meeting chip over the sidebar timeline", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.isAnchorVisible = false;
    mocks.isScrolledPastAnchor = true;
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T12:00:51.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");
    const row = screen.getByTestId("timeline-item-standup");
    const chip = container.querySelector(
      "[data-sidebar-upcoming-meeting-status]",
    ) as HTMLElement | null;

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 400,
    });
    scroller!.scrollTop = 0;
    scroller!.scrollTo = vi.fn();
    vi.spyOn(scroller!, "getBoundingClientRect").mockReturnValue({
      bottom: 400,
      height: 400,
      left: 0,
      right: 240,
      toJSON: () => ({}),
      top: 0,
      width: 240,
      x: 0,
      y: 0,
    });
    vi.spyOn(row, "getBoundingClientRect").mockReturnValue({
      bottom: 832,
      height: 32,
      left: 0,
      right: 240,
      toJSON: () => ({}),
      top: 800,
      width: 240,
      x: 0,
      y: 800,
    });

    expect(chip?.textContent).toBe("In 51s");
    expect(chip?.className).toContain("bg-destructive");
    expect(chip?.className).toContain("w-28");
    expect(chip?.querySelector("svg")).toBeTruthy();
    expect(chip?.getAttribute("aria-label")).toBe("Team standup in 51s");
    expect(screen.getByTestId("timeline-item-standup").dataset.upcoming).toBe(
      "true",
    );
    expect(
      screen.getByTestId("timeline-item-standup").dataset.upcomingLabel,
    ).toBe("In 51s");
    expect(
      screen.getByTestId("timeline-item-standup").dataset.upcomingProgress,
    ).toBe("0.17");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-12");
    expect(
      container.querySelector("[data-sidebar-timeline-top-chip-stack]")
        ?.className,
    ).toContain("top-4");
    expect(screen.queryByText("Now")).toBeNull();

    fireEvent.click(chip!);

    expect(scroller!.scrollTo).toHaveBeenCalledWith({
      top: 636,
      behavior: "smooth",
    });
  });

  it("hides the imminent meeting chip when the meeting row is visible", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T12:00:51.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");
    const row = screen.getByTestId("timeline-item-standup");

    vi.spyOn(scroller!, "getBoundingClientRect").mockReturnValue({
      bottom: 400,
      height: 400,
      left: 0,
      right: 240,
      toJSON: () => ({}),
      top: 0,
      width: 240,
      x: 0,
      y: 0,
    });
    vi.spyOn(row, "getBoundingClientRect").mockReturnValue({
      bottom: 120,
      height: 32,
      left: 0,
      right: 240,
      toJSON: () => ({}),
      top: 88,
      width: 240,
      x: 0,
      y: 88,
    });

    fireEvent.scroll(scroller!);

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]"),
    ).toBeNull();
  });

  it("shows upcoming meeting minutes with remaining seconds", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T12:01:01.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const { container } = render(<TimelineView topChromeInset />);

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]")
        ?.textContent,
    ).toBe("In 1m 1s");
  });

  it("keeps the meeting chip visible until the scheduled end time", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineEventsTable = {
      later: {
        title: "Roadmap review",
        started_at: "2024-01-15T12:06:00.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-later",
        has_recurrence_rules: false,
      },
    };

    const { container, rerender } = render(<TimelineView topChromeInset />);

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]"),
    ).toBeNull();

    vi.setSystemTime(new Date("2024-01-15T12:01:00.000Z"));
    mocks.currentTimeMs = Date.now();
    rerender(<TimelineView topChromeInset showOpenCalendarButton />);

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]")
        ?.textContent,
    ).toBe("In 5m 0s");

    vi.setSystemTime(new Date("2024-01-15T12:06:01.000Z"));
    mocks.currentTimeMs = Date.now();
    rerender(<TimelineView topChromeInset showIgnoredEvents={false} />);

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]")
        ?.textContent,
    ).toBe("Localized now");

    vi.setSystemTime(new Date("2024-01-15T12:30:01.000Z"));
    mocks.currentTimeMs = Date.now();
    rerender(
      <TimelineView
        topChromeInset
        showOpenCalendarButton
        showIgnoredEvents={false}
      />,
    );

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]"),
    ).toBeNull();
  });

  it("overlays the top now chip without reserving sidebar space", () => {
    mocks.isAnchorVisible = false;
    mocks.isScrolledPastAnchor = true;
    mocks.timelineSessionsTable = {
      past: {
        title: "Design sync",
        created_at: "2024-01-14T12:00:00.000Z",
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");

    expect(scroller).toBeInstanceOf(HTMLDivElement);

    const nowButton = screen.getByRole("button", { name: "Go back to now" });
    expect(nowButton.className).toContain("bg-card");
    expect(nowButton.className).not.toContain("backdrop-blur");
    expect(
      container.querySelector("[data-sidebar-timeline-top-chip-stack]")
        ?.className,
    ).toContain("top-4");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-12");
    expect(
      container.querySelector("[data-sidebar-timeline-bucket-header]")
        ?.className,
    ).toContain("top-12");

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(queryTopFade(container)).toBeNull();
  });

  it("places the fallback now indicator between future and past buckets", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T15:54:00.000Z"));

    mocks.configValue = "Asia/Seoul";
    mocks.timelineSessionsTable = {
      tomorrow: {
        title: "Sprint retro & planning",
        created_at: "2024-01-15T00:00:00.000Z",
        event_json: JSON.stringify({
          started_at: "2024-01-17T08:30:00.000Z",
        }),
      },
      yesterday: {
        title: "Design sync",
        created_at: "2024-01-15T12:00:00.000Z",
      },
      "two-days-ago": {
        title: "Product Discovery Pace",
        created_at: "2024-01-14T12:00:00.000Z",
      },
    };

    render(<TimelineView />);

    const tomorrowHeading = screen.getByText("Tomorrow");
    const yesterdayHeading = screen.getByText("Yesterday");
    const twoDaysAgoHeading = screen.getByText("2 days ago");
    const indicator = screen.getByTestId("current-time-indicator");

    expect(isBefore(tomorrowHeading, indicator)).toBe(true);
    expect(isBefore(indicator, yesterdayHeading)).toBe(true);
    expect(isBefore(indicator, twoDaysAgoHeading)).toBe(true);
    expect(
      indicator.closest("[data-sidebar-current-time-header-gap]")?.className,
    ).toContain("py-3");
  });

  it("does not auto-scroll to the fallback now indicator without a today bucket", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T15:54:00.000Z"));

    mocks.configValue = "UTC";
    mocks.anchorNode = document.createElement("div");
    mocks.timelineSessionsTable = {
      yesterday: {
        title: "Design sync",
        created_at: "2024-01-14T12:00:00.000Z",
      },
    };

    render(<TimelineView topChromeInset />);

    expect(screen.getByTestId("current-time-indicator")).toBeTruthy();
    expect(mocks.useAutoScrollToAnchor).toHaveBeenCalledWith(
      expect.objectContaining({
        anchorNode: null,
      }),
    );
  });

  it("auto-scrolls to the current-time anchor when a today bucket exists", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T15:54:00.000Z"));

    mocks.configValue = "UTC";
    const anchorNode = document.createElement("div");
    mocks.anchorNode = anchorNode;
    mocks.timelineSessionsTable = {
      today: {
        title: "Design sync",
        created_at: "2024-01-15T12:00:00.000Z",
      },
    };

    render(<TimelineView topChromeInset />);

    expect(screen.getByText("Today")).toBeTruthy();
    expect(mocks.useAutoScrollToAnchor).toHaveBeenCalledWith(
      expect.objectContaining({
        anchorNode,
      }),
    );
  });

  it("hides the now indicator while an active meeting is visible", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T11:15:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.liveStatus = "active";
    mocks.liveSessionId = "session-live";
    mocks.timelineSessionsTable = {
      "session-live": {
        title: "kate <> john (char)",
        created_at: "2024-01-15T11:00:00.000Z",
        event_json: JSON.stringify({
          started_at: "2024-01-15T11:00:00.000Z",
          ended_at: "2024-01-15T12:00:00.000Z",
        }),
      },
    };

    const { container } = render(<TimelineView />);

    expect(screen.getByText("Today")).toBeTruthy();
    expect(screen.getByTestId("timeline-item-session-live")).toBeTruthy();
    expect(screen.queryByTestId("current-time-indicator")).toBeNull();
    const anchor = container.querySelector(
      "[data-sidebar-current-time-anchor]",
    );
    expect(anchor).toBeTruthy();
    expect(mocks.registerAnchor).toHaveBeenCalledWith(anchor);
  });

  it("hides the now indicator while a finalizing meeting is visible", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T11:15:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.liveStatus = "finalizing";
    mocks.liveSessionId = "session-finalizing";
    mocks.timelineSessionsTable = {
      "session-finalizing": {
        title: "kate <> john (char)",
        created_at: "2024-01-15T11:00:00.000Z",
        event_json: JSON.stringify({
          started_at: "2024-01-15T11:00:00.000Z",
          ended_at: "2024-01-15T12:00:00.000Z",
        }),
      },
    };

    const { container } = render(<TimelineView />);

    expect(screen.getByText("Today")).toBeTruthy();
    expect(screen.getByTestId("timeline-item-session-finalizing")).toBeTruthy();
    expect(screen.queryByTestId("current-time-indicator")).toBeNull();
    const anchor = container.querySelector(
      "[data-sidebar-current-time-anchor]",
    );
    expect(anchor).toBeTruthy();
    expect(mocks.registerAnchor).toHaveBeenCalledWith(anchor);
  });

  it("places the fallback now indicator with fresh time after data refreshes", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T23:58:00.000Z"));
    mocks.configValue = "UTC";
    mocks.currentTimeMs = Date.now();

    const { rerender } = render(<TimelineView />);

    vi.setSystemTime(new Date("2024-01-16T00:01:00.000Z"));
    mocks.timelineSessionsTable = {
      tomorrow: {
        title: "Roadmap review",
        created_at: "2024-01-17T12:00:00.000Z",
      },
      yesterday: {
        title: "Late wrap",
        created_at: "2024-01-15T23:59:00.000Z",
      },
    };
    rerender(<TimelineView showOpenCalendarButton />);

    const tomorrowHeading = screen.getByText("Tomorrow");
    const yesterdayHeading = screen.getByText("Yesterday");
    const indicator = screen.getByTestId("current-time-indicator");

    expect(isBefore(tomorrowHeading, indicator)).toBe(true);
    expect(isBefore(indicator, yesterdayHeading)).toBe(true);
  });

  it("places the fallback now indicator after stale future buckets", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T23:58:00.000Z"));
    mocks.configValue = "UTC";
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.timelineSessionsTable = {
      soon: {
        title: "Late handoff",
        created_at: "2024-01-16T00:00:30.000Z",
      },
      yesterday: {
        title: "Planning",
        created_at: "2024-01-14T12:00:00.000Z",
      },
    };

    const { rerender } = render(<TimelineView />);

    vi.setSystemTime(new Date("2024-01-16T00:01:00.000Z"));
    mocks.currentTimeMs = Date.now();
    rerender(<TimelineView showOpenCalendarButton />);

    const staleTomorrowHeading = screen.getByText("Tomorrow");
    const staleTomorrowItem = screen.getByTestId("timeline-item-soon");
    const yesterdayHeading = screen.getByText("Yesterday");
    const indicator = screen.getByTestId("current-time-indicator");

    expect(isBefore(staleTomorrowHeading, staleTomorrowItem)).toBe(true);
    expect(isBefore(staleTomorrowItem, indicator)).toBe(true);
    expect(isBefore(indicator, yesterdayHeading)).toBe(true);
  });
});

function getSidebarActionTabsOrNull() {
  return document.querySelector("[data-sidebar-timeline-action-tabs]");
}

function queryTopFade(container: HTMLElement) {
  return container.querySelector("[data-sidebar-timeline-top-fade]");
}

function queryTopOccluder(container: HTMLElement) {
  return container.querySelector("[data-sidebar-timeline-top-occluder]");
}

function queryBottomFade(container: HTMLElement) {
  return container.querySelector("[data-sidebar-timeline-bottom-fade]");
}

function isBefore(first: Element, second: Element) {
  return Boolean(
    first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING,
  );
}
