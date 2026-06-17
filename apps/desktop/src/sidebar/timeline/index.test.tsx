import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  anchorNode: null as HTMLDivElement | null,
  openNew: vi.fn(),
  registerAnchor: vi.fn(),
  useAutoScrollToAnchor: vi.fn(),
  invalidateResource: vi.fn(),
  clearSelection: vi.fn(),
  addDeletion: vi.fn(),
  configValue: undefined as string | undefined,
  currentTimeMs: undefined as number | undefined,
  isAnchorVisible: true,
  isScrolledPastAnchor: false,
  isIgnored: vi.fn(() => false),
  liveSessionId: null as string | null,
  liveStatus: "inactive" as "inactive" | "active" | "finalizing",
  smartCurrentTimeMs: undefined as number | undefined,
  timelineEventsTable: {} as Record<string, Record<string, unknown>>,
  timelineSessionsTable: {} as Record<string, Record<string, unknown>>,
  scheduledTaskRunIds: undefined as string[] | undefined,
  runningTaskRunIds: undefined as string[] | undefined,
  taskRunInfo: {} as Record<
    string,
    { taskId: string; running: boolean; nextTimestamp: number } | undefined
  >,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.configValue,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    isIgnored: mocks.isIgnored,
  }),
}));

vi.mock("~/store/tinybase/store/deleteSession", () => ({
  captureSessionData: vi.fn(),
  deleteSessionCascade: vi.fn(),
  finalizeSessionDeletion: vi.fn(),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  QUERIES: {
    timelineEvents: "timelineEvents",
    timelineSessions: "timelineSessions",
  },
  STORE_ID: "main",
  UI: {
    useIndexes: () => null,
    useResultTable: (query: string) =>
      query === "timelineEvents"
        ? mocks.timelineEventsTable
        : mocks.timelineSessionsTable,
    useStore: () => null,
  },
}));

vi.mock("tinytick/ui-react", () => ({
  useManager: () => ({
    getTaskRunInfo: (taskRunId: string) => mocks.taskRunInfo[taskRunId],
  }),
  useRunningTaskRunIds: () => mocks.runningTaskRunIds,
  useScheduledTaskRunIds: () => mocks.scheduledTaskRunIds,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      currentTab: { type: "empty" },
      invalidateResource: mocks.invalidateResource,
      openNew: mocks.openNew,
    }),
}));

vi.mock("~/store/zustand/timeline-selection", () => ({
  useTimelineSelection: (selector: (state: unknown) => unknown) =>
    selector({
      clear: mocks.clearSelection,
      selectedIds: [],
    }),
}));

vi.mock("~/store/zustand/undo-delete", () => ({
  useUndoDelete: (selector: (state: unknown) => unknown) =>
    selector({
      addDeletion: mocks.addDeletion,
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
  TimelineItemComponent: ({ item }: { item: { id: string } }) => (
    <div data-testid={`timeline-item-${item.id}`} />
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
    mocks.smartCurrentTimeMs = undefined;
    mocks.timelineEventsTable = {};
    mocks.timelineSessionsTable = {};
    mocks.scheduledTaskRunIds = undefined;
    mocks.runningTaskRunIds = undefined;
    mocks.taskRunInfo = {};
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
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-20");

    fireEvent.click(calendarButton);

    expect(mocks.openNew).toHaveBeenCalledWith({ type: "calendar" });
  });

  it("shows a due calendar sync status in sidebar chrome", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.scheduledTaskRunIds = ["calendar-sync-run"];
    mocks.taskRunInfo = {
      "calendar-sync-run": {
        taskId: "calendarSync",
        running: false,
        nextTimestamp: Date.now(),
      },
    };

    const { container } = render(<TimelineView topChromeInset />);

    expect(screen.getByRole("status").textContent).toBe(
      "Starting calendar sync",
    );
    expect(screen.getByRole("status").className).toContain("rounded-full");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-20");
  });

  it("updates scheduled calendar sync status as the due window passes", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.scheduledTaskRunIds = ["calendar-sync-run"];
    mocks.taskRunInfo = {
      "calendar-sync-run": {
        taskId: "calendarSync",
        running: false,
        nextTimestamp: Date.now() + 2000,
      },
    };

    const { rerender } = render(<TimelineView topChromeInset />);

    expect(screen.queryByRole("status")).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    mocks.currentTimeMs = Date.now();
    rerender(<TimelineView topChromeInset />);

    expect(screen.getByRole("status").textContent).toBe(
      "Starting calendar sync",
    );

    act(() => {
      vi.advanceTimersByTime(2500);
    });
    mocks.currentTimeMs = Date.now();
    rerender(<TimelineView topChromeInset />);

    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows a running calendar sync status in sidebar chrome", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.runningTaskRunIds = ["calendar-sync-run"];
    mocks.taskRunInfo = {
      "calendar-sync-run": {
        taskId: "calendarSync",
        running: true,
        nextTimestamp: Date.now() + 1000,
      },
    };

    render(<TimelineView topChromeInset />);

    expect(screen.getByRole("status").textContent).toBe("Syncing calendar");
  });

  it("hides calendar sync status while visible events are listed", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.now();
    mocks.smartCurrentTimeMs = Date.now();
    mocks.runningTaskRunIds = ["calendar-sync-run"];
    mocks.taskRunInfo = {
      "calendar-sync-run": {
        taskId: "calendarSync",
        running: true,
        nextTimestamp: Date.now() + 1000,
      },
    };
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T12:30:00.000Z",
        ended_at: "2024-01-15T13:00:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    render(<TimelineView topChromeInset />);

    expect(screen.getByTestId("timeline-item-standup")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("keeps calendar sync status visible while scrolled", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.runningTaskRunIds = ["calendar-sync-run"];
    mocks.taskRunInfo = {
      "calendar-sync-run": {
        taskId: "calendarSync",
        running: true,
        nextTimestamp: Date.now() + 1000,
      },
    };

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");

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

    expect(screen.getByRole("status").textContent).toBe("Syncing calendar");
    expect(getSidebarActionTabsOrNull()).toBeNull();
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-20");
  });

  it("hides future repeated calendar sync schedules from sidebar chrome", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.scheduledTaskRunIds = ["calendar-sync-run"];
    mocks.taskRunInfo = {
      "calendar-sync-run": {
        taskId: "calendarSync",
        running: false,
        nextTimestamp: Date.now() + 60_000,
      },
    };

    render(<TimelineView topChromeInset />);

    expect(screen.queryByRole("status")).toBeNull();
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
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-10");

    fireEvent.click(calendarButton);

    expect(mocks.openNew).toHaveBeenCalledWith({ type: "calendar" });
  });

  it("keeps top chrome compact while scrolled without timeline action tabs", () => {
    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");

    expect(scroller).toBeInstanceOf(HTMLDivElement);
    expect(getSidebarActionTabsOrNull()).toBeNull();

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
    expect(getTopFade(container).className).toContain("h-16");
    expect(getTopFade(container).className).toContain("from-60%");
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

    expect(scroller!.style.maskImage).toBe(
      "linear-gradient(to bottom, #000 0, #000 calc(100% - 28px), transparent 100%)",
    );
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

    expect(scroller!.style.maskImage).toBe(
      "linear-gradient(to bottom, transparent 0, #000 28px, #000 100%)",
    );
  });

  it("shows an imminent meeting chip over the sidebar timeline", () => {
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
    const chip = container.querySelector(
      "[data-sidebar-upcoming-meeting-status]",
    );

    expect(chip?.textContent).toBe("Starts in 51s");
    expect(chip?.className).toContain("bg-destructive");
    expect(chip?.getAttribute("aria-label")).toBe("Team standup starts in 51s");
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-12");
  });

  it("rounds upcoming meeting minutes down to elapsed whole minutes", () => {
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
    ).toBe("Starts in 1m");
  });

  it("hides the imminent meeting chip outside the start window", () => {
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
    rerender(<TimelineView topChromeInset />);

    expect(
      container.querySelector("[data-sidebar-upcoming-meeting-status]")
        ?.textContent,
    ).toBe("Starts in 5m");

    vi.setSystemTime(new Date("2024-01-15T12:06:01.000Z"));
    mocks.currentTimeMs = Date.now();
    rerender(<TimelineView topChromeInset />);

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

    expect(screen.getByRole("button", { name: "Go back to now" })).toBeTruthy();
    expect(
      container.querySelector("[data-sidebar-timeline-top-spacer]")?.className,
    ).toContain("h-12");
    expect(
      container.querySelector("[data-sidebar-timeline-bucket-header]")
        ?.className,
    ).toContain("top-12");
    expect(getTopFade(container).className).toContain("h-16");
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
    rerender(<TimelineView />);

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
    rerender(<TimelineView />);

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

function getTopFade(container: HTMLElement) {
  const topFade = container.querySelector("[data-sidebar-timeline-top-fade]");

  expect(topFade).toBeInstanceOf(HTMLDivElement);

  return topFade as HTMLDivElement;
}

function isBefore(first: Element, second: Element) {
  return Boolean(
    first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING,
  );
}
