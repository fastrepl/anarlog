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
  activatedSessionIds: new Set<string>(),
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

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "owner-1" } } }),
}));

vi.mock("~/shared-notes/cache", () => ({
  useActivatedSessionShareIds: () => mocks.activatedSessionIds,
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
  ManagedSharedSessionIdsContext: {
    Provider: ({ children }: { children: ReactNode }) => <>{children}</>,
  },
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

import { resetSidebarNotes, useSidebarNotes } from "~/sidebar/note-filter";

const twoSelectableNotes = {
  "selected-note": {
    title: "Selected note",
    created_at: "2024-01-15T12:00:00.000Z",
  },
  "other-note": {
    title: "Other note",
    created_at: "2024-01-15T11:00:00.000Z",
  },
};

const standupEvent = (startedAt: string) => ({
  standup: {
    title: "Team standup",
    started_at: startedAt,
    ended_at: "2024-01-15T12:30:00.000Z",
    tracking_id_event: "event-standup",
    has_recurrence_rules: false,
  },
});

function freezeTime(iso: string) {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(iso));
  mocks.currentTimeMs = Date.now();
  mocks.smartCurrentTimeMs = Date.now();
}

function mountEditor() {
  const editor = document.createElement("div");
  editor.className = "ProseMirror";
  editor.contentEditable = "true";
  editor.tabIndex = 0;
  document.body.appendChild(editor);
  editor.focus();
  return editor;
}

function rect(top: number, height: number): DOMRect {
  return {
    bottom: top + height,
    height,
    left: 0,
    right: 240,
    toJSON: () => ({}),
    top,
    width: 240,
    x: 0,
    y: top,
  };
}

function queryMeetingChip(container: HTMLElement) {
  return container.querySelector<HTMLElement>(
    "[data-sidebar-upcoming-meeting-status]",
  );
}

function isBefore(first: Element, second: Element) {
  return Boolean(
    first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING,
  );
}

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
    mocks.smartCurrentTimeMs = undefined;
    mocks.timelineSelectionAnchorId = null;
    mocks.timelineSelectionSelectedIds = [];
    mocks.timelineEventsTable = {};
    mocks.timelineSessionsTable = {};
    mocks.activatedSessionIds = new Set();
  });

  afterEach(() => {
    cleanup();
    resetSidebarNotes();
    vi.useRealTimers();
  });

  it("opens the calendar from the timeline chip", () => {
    freezeTime("2024-01-15T12:00:00.000Z");
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
      },
    };

    render(<TimelineView topChromeInset />);
    fireEvent.click(screen.getByRole("button", { name: "Open calendar" }));

    expect(mocks.openNew).toHaveBeenCalledWith({ type: "calendar" });
  });

  it("selects all visible notes with Cmd+A after a sidebar note selection", () => {
    freezeTime("2024-01-15T09:00:00.000Z");
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
    mocks.timelineSessionsTable = twoSelectableNotes;

    render(<TimelineView />);
    fireEvent.keyDown(window, { key: "a", metaKey: true });

    expect(mocks.selectAll).toHaveBeenCalledWith([
      "session-selected-note",
      "session-other-note",
    ]);
  });

  it("does not select sidebar notes while the mounted timeline is hidden", () => {
    freezeTime("2024-01-15T09:00:00.000Z");
    mocks.currentTab = { type: "sessions", id: "selected-note" };
    mocks.timelineSelectionAnchorId = "session-selected-note";
    mocks.timelineSessionsTable = twoSelectableNotes;

    render(
      <div aria-hidden inert>
        <TimelineView />
      </div>,
    );
    fireEvent.keyDown(window, { key: "a", metaKey: true });

    expect(mocks.selectAll).not.toHaveBeenCalled();
  });

  it.each([
    ["Backspace", { key: "Backspace" }],
    ["Cmd+A", { key: "a", metaKey: true }],
  ])("leaves %s to the focused editor", (_, keyEvent) => {
    freezeTime("2024-01-15T09:00:00.000Z");
    mocks.currentTab = { type: "sessions", id: "selected-note" };
    mocks.timelineSelectionAnchorId = "session-selected-note";
    mocks.timelineSelectionSelectedIds = [
      "session-selected-note",
      "session-other-note",
    ];
    mocks.timelineSessionsTable = twoSelectableNotes;

    render(<TimelineView />);
    const editor = mountEditor();
    fireEvent.keyDown(editor, keyEvent);

    expect(mocks.deleteSession).not.toHaveBeenCalled();
    expect(mocks.selectAll).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();

    editor.remove();
  });

  it.each(["Backspace", "Delete"])(
    "confirms selected note deletion with %s",
    (key) => {
      freezeTime("2024-01-15T09:00:00.000Z");
      mocks.timelineSelectionSelectedIds = [
        "session-selected-note",
        "session-other-note",
      ];
      mocks.timelineSessionsTable = twoSelectableNotes;

      render(<TimelineView />);
      fireEvent.keyDown(window, { key });

      expect(mocks.deleteSession).not.toHaveBeenCalled();
      expect(
        screen.getByRole("heading", { name: "Delete 2 selected notes?" }),
      ).toBeTruthy();

      fireEvent.click(screen.getByRole("button", { name: "Delete" }));

      expect(mocks.deleteSession).toHaveBeenCalledWith("selected-note", {
        batchId: expect.any(String),
        title: "Selected note",
      });
      expect(mocks.deleteSession).toHaveBeenCalledWith("other-note", {
        batchId: expect.any(String),
        title: "Other note",
      });
      expect(mocks.deleteSession.mock.calls[0]![1].batchId).toBe(
        mocks.deleteSession.mock.calls[1]![1].batchId,
      );
      expect(mocks.clearSelection).toHaveBeenCalledOnce();
    },
  );

  it("keeps selected notes when deletion is canceled", () => {
    mocks.timelineSelectionSelectedIds = ["session-selected-note"];
    mocks.timelineSessionsTable = {
      "selected-note": twoSelectableNotes["selected-note"],
    };

    render(<TimelineView />);
    fireEvent.keyDown(window, { key: "Backspace" });
    fireEvent.pointerDown(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mocks.deleteSession).not.toHaveBeenCalled();
    expect(mocks.clearSelection).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("clears selected notes when clicking main pane actions", () => {
    mocks.timelineSelectionSelectedIds = [
      "session-selected-note",
      "session-other-note",
    ];

    render(
      <>
        <TimelineView />
        <button type="button">Transcript</button>
      </>,
    );
    fireEvent.pointerDown(screen.getByRole("button", { name: "Transcript" }));

    expect(mocks.clearSelection).toHaveBeenCalledOnce();
  });

  it("shows an imminent meeting chip that scrolls to the meeting row", () => {
    freezeTime("2024-01-15T12:00:00.000Z");
    mocks.isAnchorVisible = false;
    mocks.isScrolledPastAnchor = true;
    mocks.timelineEventsTable = standupEvent("2024-01-15T12:00:51.000Z");

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]")!;
    const row = screen.getByTestId("timeline-item-standup");
    const chip = queryMeetingChip(container);

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 400,
    });
    scroller.scrollTop = 0;
    scroller.scrollTo = vi.fn();
    vi.spyOn(scroller, "getBoundingClientRect").mockReturnValue(rect(0, 400));
    vi.spyOn(row, "getBoundingClientRect").mockReturnValue(rect(800, 32));

    expect(chip?.getAttribute("aria-label")).toBe("Team standup in 51s");
    expect(row.dataset).toMatchObject({
      upcoming: "true",
      upcomingLabel: "In 51s",
      upcomingProgress: "0.17",
    });

    fireEvent.click(chip!);

    expect(scroller.scrollTo).toHaveBeenCalledWith({
      top: 636,
      behavior: "smooth",
    });
  });

  it("hides the imminent meeting chip when the meeting row is visible", () => {
    freezeTime("2024-01-15T12:00:00.000Z");
    mocks.timelineEventsTable = standupEvent("2024-01-15T12:00:51.000Z");

    const { container } = render(<TimelineView topChromeInset />);
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]")!;
    vi.spyOn(scroller, "getBoundingClientRect").mockReturnValue(rect(0, 400));
    vi.spyOn(
      screen.getByTestId("timeline-item-standup"),
      "getBoundingClientRect",
    ).mockReturnValue(rect(88, 32));

    fireEvent.scroll(scroller);

    expect(queryMeetingChip(container)).toBeNull();
  });

  it("keeps the meeting chip visible until the scheduled end time", () => {
    freezeTime("2024-01-15T12:00:00.000Z");
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
    const advanceTo = (iso: string, props: object) => {
      vi.setSystemTime(new Date(iso));
      mocks.currentTimeMs = Date.now();
      fireEvent.focus(window);
      rerender(<TimelineView topChromeInset {...props} />);
    };

    expect(queryMeetingChip(container)).toBeNull();

    advanceTo("2024-01-15T12:01:00.000Z", { showOpenCalendarButton: true });
    expect(queryMeetingChip(container)?.textContent).toBe("In 5m 0s");

    advanceTo("2024-01-15T12:06:01.000Z", { showIgnoredEvents: false });
    expect(queryMeetingChip(container)?.textContent).toBe("Localized now");

    advanceTo("2024-01-15T12:30:01.000Z", {
      showOpenCalendarButton: true,
      showIgnoredEvents: false,
    });
    expect(queryMeetingChip(container)).toBeNull();
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
    const indicator = screen.getByTestId("current-time-indicator");

    expect(isBefore(screen.getByText("Tomorrow"), indicator)).toBe(true);
    expect(isBefore(indicator, screen.getByText("Yesterday"))).toBe(true);
    expect(isBefore(indicator, screen.getByText("2 days ago"))).toBe(true);
  });

  it("places the fallback now indicator after stale future buckets", () => {
    freezeTime("2024-01-15T23:58:00.000Z");
    mocks.configValue = "UTC";
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

    const staleTomorrowItem = screen.getByTestId("timeline-item-soon");
    const indicator = screen.getByTestId("current-time-indicator");

    expect(isBefore(screen.getByText("Tomorrow"), staleTomorrowItem)).toBe(
      true,
    );
    expect(isBefore(staleTomorrowItem, indicator)).toBe(true);
    expect(isBefore(indicator, screen.getByText("Yesterday"))).toBe(true);
  });

  it.each([
    ["without", false, { yesterday: "2024-01-14T12:00:00.000Z" }],
    ["with", true, { today: "2024-01-15T12:00:00.000Z" }],
  ])(
    "auto-scrolls to the now anchor only %s a today bucket",
    (_, hasToday, sessions) => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2024-01-15T15:54:00.000Z"));
      mocks.configValue = "UTC";
      const anchorNode = document.createElement("div");
      mocks.anchorNode = anchorNode;
      mocks.timelineSessionsTable = Object.fromEntries(
        Object.entries(sessions).map(([id, createdAt]) => [
          id,
          { title: "Design sync", created_at: createdAt },
        ]),
      );

      render(<TimelineView topChromeInset />);

      expect(mocks.useAutoScrollToAnchor).toHaveBeenCalledWith(
        expect.objectContaining({ anchorNode: hasToday ? anchorNode : null }),
      );
    },
  );

  it.each(["active", "finalizing"] as const)(
    "anchors to a visible %s meeting instead of the now indicator",
    (status) => {
      freezeTime("2024-01-15T11:15:00.000Z");
      mocks.liveStatus = status;
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

      expect(screen.getByTestId("timeline-item-session-live")).toBeTruthy();
      expect(screen.queryByTestId("current-time-indicator")).toBeNull();
      const anchor = container.querySelector(
        "[data-sidebar-current-time-anchor]",
      );
      expect(anchor).toBeTruthy();
      expect(mocks.registerAnchor).toHaveBeenCalledWith(anchor);
    },
  );

  it("groups notes by folder and hides the date timeline chrome", () => {
    freezeTime("2024-01-15T12:00:00.000Z");
    mocks.timelineSessionsTable = {
      later: {
        title: "Quarterly planning",
        created_at: "2024-01-17T12:00:00.000Z",
        folder_id: "work",
      },
      notes: {
        title: "Scratch",
        created_at: "2024-01-15T11:00:00.000Z",
        folder_id: "",
      },
    };
    useSidebarNotes.getState().setGroupBy("folder");

    render(<TimelineView topChromeInset />);

    expect(screen.getByText("work")).toBeTruthy();
    expect(screen.getByText("No folder")).toBeTruthy();
    expect(screen.queryByText("Today")).toBeNull();
    expect(screen.queryByRole("button", { name: "Open calendar" })).toBeNull();
    expect(screen.queryByTestId("current-time-indicator")).toBeNull();
  });
});
