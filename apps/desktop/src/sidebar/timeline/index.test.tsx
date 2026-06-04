import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  newNote: vi.fn(),
  openSearch: vi.fn(),
  openNew: vi.fn(),
  invalidateResource: vi.fn(),
  clearSelection: vi.fn(),
  addDeletion: vi.fn(),
  configValue: undefined as string | undefined,
  currentTimeMs: undefined as number | undefined,
  smartCurrentTimeMs: undefined as number | undefined,
  timelineEventsTable: {} as Record<string, Record<string, unknown>>,
  timelineSessionsTable: {} as Record<string, Record<string, unknown>>,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.configValue,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.newNote,
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({
    open: mocks.openSearch,
  }),
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    isIgnored: () => false,
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

vi.mock("./anchor", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    useAnchor: () => ({
      anchorNode: null,
      containerRef: React.useRef<HTMLDivElement>(null),
      isAnchorVisible: true,
      isScrolledPastAnchor: false,
      registerAnchor: vi.fn(),
      scrollToAnchor: vi.fn(),
    }),
    useAutoScrollToAnchor: vi.fn(),
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
    mocks.configValue = undefined;
    mocks.currentTimeMs = undefined;
    mocks.smartCurrentTimeMs = undefined;
    mocks.timelineEventsTable = {};
    mocks.timelineSessionsTable = {};
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("shows sidebar actions for creating and searching notes", () => {
    render(<TimelineView topChromeInset />);

    fireEvent.click(screen.getByRole("button", { name: "New note" }));
    fireEvent.click(screen.getByRole("button", { name: "Search" }));

    expect(mocks.newNote).toHaveBeenCalledTimes(1);
    expect(mocks.openSearch).toHaveBeenCalledTimes(1);
  });

  it("places the open calendar chip below visible sidebar actions", () => {
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

    render(<TimelineView topChromeInset />);

    expect(getSidebarActions().className).not.toContain("opacity-0");
    expect(
      screen.getByRole("button", { name: "Open calendar" }).parentElement
        ?.className,
    ).toContain("top-36");
  });

  it("hides sidebar actions briefly after scrolling down", () => {
    vi.useFakeTimers();

    const { container } = render(<TimelineView topChromeInset />);
    const actions = getSidebarActions();
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");

    expect(scroller).toBeInstanceOf(HTMLDivElement);
    expect(actions.className).not.toContain("opacity-0");

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

    expect(actions.className).toContain("opacity-0");
    expect(getTopFade(container).className).toContain("h-20");

    act(() => {
      vi.advanceTimersByTime(900);
    });

    expect(actions.className).not.toContain("opacity-0");
    expect(getTopFade(container).className).toContain("h-36");
  });

  it("keeps sidebar actions hidden during timeline data refreshes", () => {
    vi.useFakeTimers();

    const { container, rerender } = render(<TimelineView topChromeInset />);
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

    expect(getSidebarActions().className).toContain("opacity-0");

    mocks.timelineSessionsTable = {
      "session-1": {
        title: "Planning",
        created_at: "2024-01-15T12:00:00.000Z",
      },
    };
    rerender(<TimelineView topChromeInset />);

    expect(getSidebarActions().className).toContain("opacity-0");

    act(() => {
      vi.advanceTimersByTime(900);
    });

    expect(getSidebarActions().className).not.toContain("opacity-0");
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

function getSidebarActions() {
  const actions = screen
    .getByRole("button", { name: "New note" })
    .closest("[data-sidebar-timeline-actions]");

  expect(actions).toBeInstanceOf(HTMLDivElement);

  return actions as HTMLDivElement;
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
