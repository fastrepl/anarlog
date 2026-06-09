import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNewNote: vi.fn(),
  liveSessionId: null as string | null,
  openNew: vi.fn(),
  startDragging: vi.fn().mockResolvedValue(undefined),
  stopListening: vi.fn(),
  sessionModes: {} as Record<string, string>,
  timelineEventsTable: {},
  timelineSessionsTable: {},
  timelineTranscriptsTable: {} as Record<
    string,
    {
      ended_at?: number | null;
      session_id?: string | null;
      started_at?: number | null;
      words?: string | null;
    }
  >,
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    startDragging: mocks.startDragging,
  }),
}));

vi.mock("~/session/components/session-preview-card", () => ({
  SessionPreviewCard: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <div data-testid="timeline-spinner" />,
}));

vi.mock("~/session/hooks/useEnhancedNotes", () => ({
  useIsSessionEnhancing: () => false,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => undefined,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.createNewNote,
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: vi.fn(),
    ignoreSeries: vi.fn(),
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
    useCell: () => undefined,
    useIndexes: () => undefined,
    useResultTable: (query: string) => {
      if (query === "timelineEvents") {
        return mocks.timelineEventsTable;
      }

      if (query === "timelineSessions") {
        return mocks.timelineSessionsTable;
      }

      return {};
    },
    useRow: () => undefined,
    useStore: () => undefined,
    useTable: (table: string) =>
      table === "transcripts" ? mocks.timelineTranscriptsTable : {},
  },
}));

vi.mock("~/store/tinybase/store/sessions", () => ({
  getOrCreateSessionForEventId: vi.fn(),
}));

vi.mock("~/store/zustand/live-title", () => ({
  useSessionTitle: () => undefined,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      openNew: mocks.openNew,
    }),
  ),
}));

vi.mock("~/store/zustand/undo-delete", () => ({
  useUndoDelete: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      addDeletion: vi.fn(),
    }),
  ),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      getSessionMode: (sessionId: string) =>
        mocks.sessionModes[sessionId] ?? "inactive",
      live: {
        amplitude: { mic: 0.5, speaker: 0.25 },
        sessionId: mocks.liveSessionId,
      },
      stop: mocks.stopListening,
    }),
  ),
}));

import {
  formatTimelineStartLabel,
  getTimelineCarouselNowDirection,
  TopMeetingTimeline,
} from "~/main/top-meeting-timeline";

describe("TopMeetingTimeline", () => {
  beforeEach(() => {
    mocks.createNewNote.mockClear();
    mocks.openNew.mockClear();
    mocks.startDragging.mockClear();
    mocks.stopListening.mockClear();
    mocks.liveSessionId = null;
    mocks.sessionModes = {};
    mocks.timelineEventsTable = {};
    mocks.timelineSessionsTable = {};
    mocks.timelineTranscriptsTable = {};
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("keeps timeline clicks working when the pointer does not drag", () => {
    render(<TopMeetingTimeline currentTab={null} />);

    const createButton = screen.getByRole("button", {
      name: /Create new note/,
    });

    fireEvent.pointerDown(createButton, {
      button: 0,
      clientX: 10,
      clientY: 10,
      pointerId: 1,
    });
    fireEvent.click(createButton);

    expect(mocks.startDragging).not.toHaveBeenCalled();
    expect(mocks.createNewNote).toHaveBeenCalledTimes(1);
  });

  it("starts window drag and ignores the release click after dragging", () => {
    render(<TopMeetingTimeline currentTab={null} />);

    const createButton = screen.getByRole("button", {
      name: /Create new note/,
    });

    fireEvent.pointerDown(createButton, {
      button: 0,
      clientX: 10,
      clientY: 10,
      pointerId: 1,
    });
    fireEvent.pointerMove(createButton, {
      clientX: 18,
      clientY: 10,
      pointerId: 1,
    });
    fireEvent.click(createButton);

    expect(mocks.startDragging).toHaveBeenCalledTimes(1);
    expect(mocks.createNewNote).not.toHaveBeenCalled();
  });

  it("shows timeline item titles above start time metadata", () => {
    const start = new Date();
    const startLabel = formatTimelineStartLabel(start);

    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: start.toISOString(),
        event_json: "",
        title: "Design Review",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const title = screen.getByText("Design Review");
    const cardButton = title.closest("button");
    const startMetadata = within(cardButton!).getByText(startLabel);
    const buttonText = cardButton?.textContent ?? "";

    expect(cardButton).toBe(startMetadata.closest("button"));
    expect(buttonText.indexOf("Design Review")).toBeLessThan(
      buttonText.indexOf(startLabel),
    );
    expect(startLabel).not.toContain("-");
  });

  it("highlights the selected session with accent styling", () => {
    const start = new Date();
    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: start.toISOString(),
        event_json: "",
        title: "Selected Meeting",
      },
      "session-2": {
        created_at: new Date(start.getTime() + 60 * 60 * 1000).toISOString(),
        event_json: "",
        title: "Other Meeting",
      },
    };

    render(
      <TopMeetingTimeline
        currentTab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: null },
          type: "sessions",
        }}
      />,
    );

    const selectedButton = screen
      .getByText("Selected Meeting")
      .closest("button");
    const otherButton = screen.getByText("Other Meeting").closest("button");
    const selectedClasses = selectedButton?.className.split(/\s+/) ?? [];
    const otherClasses = otherButton?.className.split(/\s+/) ?? [];

    expect(selectedClasses).toContain("bg-accent");
    expect(selectedClasses).toContain("border-ring");
    expect(selectedClasses).not.toContain("bg-primary");
    expect(otherClasses).toContain("bg-card");
    expect(otherClasses).not.toContain("bg-accent");
  });

  it("shows active meetings as red with a stop suffix", () => {
    const start = new Date();
    mocks.liveSessionId = "session-1";
    mocks.sessionModes = { "session-1": "active" };
    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: start.toISOString(),
        event_json: "",
        title: "Live Meeting",
      },
    };

    render(
      <TopMeetingTimeline
        currentTab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: null },
          type: "sessions",
        }}
      />,
    );

    const title = screen.getByText("Live Meeting");
    const cardButton = title.closest("button");
    const stopButton = screen.getByLabelText("Stop listening");

    expect(cardButton?.className).toContain("bg-destructive");
    expect(cardButton?.className).not.toContain("bg-primary");

    fireEvent.click(stopButton);

    expect(mocks.stopListening).toHaveBeenCalledTimes(1);
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  it("shows processing sessions with a spinner suffix", () => {
    const start = new Date();
    mocks.sessionModes = { "session-1": "running_batch" };
    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: start.toISOString(),
        event_json: "",
        title: "Processing Meeting",
      },
    };

    render(
      <TopMeetingTimeline
        currentTab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: null },
          type: "sessions",
        }}
      />,
    );

    const title = screen.getByText("Processing Meeting");
    const cardButton = title.closest("button");
    const spinnerSuffix = screen.getByRole("status", {
      name: "Loading timeline item",
    });

    expect(cardButton?.className).toContain("pr-8");
    expect(spinnerSuffix.className).toContain("text-muted-foreground");
    expect(screen.getAllByTestId("timeline-spinner")).toHaveLength(1);
    expect(within(cardButton!).queryByTestId("timeline-spinner")).toBeNull();
  });

  it("shows the current time marker inside active timeline blocks", () => {
    const now = new Date("2026-05-29T15:00:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.timelineEventsTable = {
      "event-1": {
        calendar_id: null,
        ended_at: new Date(now.getTime() + 60 * 60 * 1000).toISOString(),
        has_recurrence_rules: false,
        started_at: new Date(now.getTime() - 60 * 60 * 1000).toISOString(),
        title: "Active Event",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const indicator = screen.getByTestId("top-timeline-now-indicator");

    expect(indicator.style.left).toBe("80px");
    expect(indicator.children).toHaveLength(2);
    expect(indicator.children[0]?.className).toContain("top-0");
    expect(indicator.children[0]?.className).toContain("bottom-1");
  });

  it("uses the recording end for completed sessions linked to calendar events", () => {
    const now = new Date("2026-05-29T11:30:00.000Z");
    const recordingStart = new Date("2026-05-29T10:00:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: recordingStart.toISOString(),
        event_json: JSON.stringify({
          calendar_id: null,
          ended_at: new Date("2026-05-29T11:00:00.000Z").toISOString(),
          started_at: recordingStart.toISOString(),
          title: "Long-running sync",
        }),
        title: "Long-running sync",
      },
    };
    mocks.timelineTranscriptsTable = {
      "transcript-1": {
        ended_at: new Date("2026-05-29T13:00:00.000Z").getTime(),
        session_id: "session-1",
        started_at: recordingStart.getTime(),
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const card = screen
      .getByText("Long-running sync")
      .closest("[data-timeline-start-ms]") as HTMLDivElement | null;
    const cardWidth = Number.parseFloat(card?.style.width ?? "");
    const indicatorX = Number.parseFloat(
      screen.getByTestId("top-timeline-now-indicator").style.left,
    );

    expect(indicatorX).toBe(cardWidth / 2);
  });

  it("places the create note card next to the latest note instead of a future event", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: new Date("2026-05-29T15:28:00.000Z").toISOString(),
        event_json: "",
        title: "Untitled",
      },
    };
    mocks.timelineEventsTable = {
      "event-1": {
        calendar_id: null,
        ended_at: new Date("2026-05-29T18:00:00.000Z").toISOString(),
        has_recurrence_rules: false,
        started_at: new Date("2026-05-29T17:30:00.000Z").toISOString(),
        title: "Design sync",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const createButton = screen.getByRole("button", {
      name: /Create new note/,
    });
    const createCard = createButton.closest(
      "[data-timeline-start-ms]",
    ) as HTMLDivElement | null;
    const cardWidth = Number.parseFloat(createCard?.style.width ?? "");
    const carousel = createCard?.parentElement as HTMLDivElement | null;
    const timelineCards = Array.from(
      document.querySelectorAll<HTMLElement>("[data-timeline-start-ms]"),
    );
    const indicatorX = Number.parseFloat(
      screen.getByTestId("top-timeline-now-indicator").style.left,
    );

    expect(timelineCards[0]?.textContent).toContain("Untitled");
    expect(timelineCards[1]?.textContent).toContain("Create new note");
    expect(timelineCards[2]?.textContent).toContain("Design sync");
    expect(createCard?.textContent).not.toContain("Today");
    expect(createCard?.textContent).not.toContain("3:41 PM");
    expect(cardWidth).toBe(160);
    expect(carousel?.style.width).toBe("512px");
    expect(indicatorX).toBe(164);
  });

  it("keeps same-day events before the current-time create note card", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: new Date("2026-05-29T15:28:00.000Z").toISOString(),
        event_json: "",
        title: "Untitled",
      },
    };
    mocks.timelineEventsTable = {
      "event-1": {
        calendar_id: null,
        ended_at: new Date("2026-05-29T16:00:00.000Z").toISOString(),
        has_recurrence_rules: false,
        started_at: new Date("2026-05-29T15:33:00.000Z").toISOString(),
        title: "Post-note event",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    screen.getByRole("button", { name: /Create new note/ });
    const timelineCards = Array.from(
      document.querySelectorAll<HTMLElement>("[data-timeline-start-ms]"),
    );

    expect(timelineCards[0]?.textContent).toContain("Untitled");
    expect(timelineCards[1]?.textContent).toContain("Post-note event");
    expect(timelineCards[2]?.textContent).toContain("Create new note");
  });

  it("places the create note card before future notes", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: new Date("2026-05-29T15:28:00.000Z").toISOString(),
        event_json: "",
        title: "Current note",
      },
      "session-2": {
        created_at: new Date("2026-05-29T16:15:00.000Z").toISOString(),
        event_json: "",
        title: "Future note",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const timelineCards = Array.from(
      document.querySelectorAll<HTMLElement>("[data-timeline-start-ms]"),
    );

    expect(timelineCards[0]?.textContent).toContain("Current note");
    expect(timelineCards[1]?.textContent).toContain("Create new note");
    expect(timelineCards[2]?.textContent).toContain("Future note");
  });

  it("places a newly created note before the create note card before the clock ticks", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    const createdAt = new Date("2026-05-29T15:41:30.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    const { rerender } = render(<TopMeetingTimeline currentTab={null} />);

    vi.setSystemTime(createdAt);
    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: createdAt.toISOString(),
        event_json: "",
        title: "Untitled",
      },
    };

    rerender(<TopMeetingTimeline currentTab={null} />);

    const timelineCards = Array.from(
      document.querySelectorAll<HTMLElement>("[data-timeline-start-ms]"),
    );

    expect(timelineCards[0]?.textContent).toContain("Untitled");
    expect(timelineCards[1]?.textContent).toContain("Create new note");
  });

  it("keeps manual scroll when the trailing create note clock ticks", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(<TopMeetingTimeline currentTab={null} />);

    const createCard = screen
      .getByRole("button", { name: /Create new note/ })
      .closest("[data-timeline-start-ms]") as HTMLDivElement | null;
    const scrollContainer = createCard?.parentElement?.parentElement;

    expect(scrollContainer).toBeTruthy();

    Object.defineProperty(scrollContainer, "clientWidth", {
      configurable: true,
      value: 100,
    });
    scrollContainer!.scrollLeft = 42;

    act(() => {
      vi.setSystemTime(new Date(now.getTime() + 60_100));
      vi.advanceTimersByTime(60_100);
    });

    expect(scrollContainer!.scrollLeft).toBe(42);
  });

  it("keeps create note visible next to active ad-hoc meetings", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.liveSessionId = "session-1";
    mocks.sessionModes = { "session-1": "active" };
    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: new Date("2026-05-29T15:28:00.000Z").toISOString(),
        event_json: "",
        title: "Live Ad-hoc",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const timelineCards = Array.from(
      document.querySelectorAll<HTMLElement>("[data-timeline-start-ms]"),
    );

    expect(timelineCards[0]?.textContent).toContain("Live Ad-hoc");
    expect(timelineCards[1]?.textContent).toContain("Create new note");
    expect(
      screen.getByRole("button", { name: /Create new note/ }),
    ).toBeTruthy();
    expect(screen.queryByTestId("top-timeline-now-indicator")).toBeNull();
  });

  it("shows the now chip on the left when the current time marker is behind the viewport", () => {
    expect(
      getTimelineCarouselNowDirection({
        nowX: 190,
        scrollLeft: 250,
        viewportWidth: 100,
      }),
    ).toBe("left");
  });

  it("scrolls the now chip to the current time marker", () => {
    const now = new Date("2026-05-29T15:41:00.000Z");
    vi.useFakeTimers();
    vi.setSystemTime(now);

    mocks.timelineSessionsTable = {
      "session-1": {
        created_at: new Date("2026-05-29T15:28:00.000Z").toISOString(),
        event_json: "",
        title: "Untitled",
      },
    };
    mocks.timelineEventsTable = {
      "event-1": {
        calendar_id: null,
        ended_at: new Date("2026-05-29T18:00:00.000Z").toISOString(),
        has_recurrence_rules: false,
        started_at: new Date("2026-05-29T17:30:00.000Z").toISOString(),
        title: "Design sync",
      },
    };

    render(<TopMeetingTimeline currentTab={null} />);

    const indicator = screen.getByTestId("top-timeline-now-indicator");
    const scrollContainer = indicator.parentElement?.parentElement;
    expect(scrollContainer).toBeTruthy();

    Object.defineProperty(scrollContainer, "clientWidth", {
      configurable: true,
      value: 100,
    });
    scrollContainer!.scrollLeft = 380;
    fireEvent.scroll(scrollContainer!);

    fireEvent.click(screen.getByRole("button", { name: "Now" }));

    expect(scrollContainer!.scrollLeft).toBe(114);
  });
});
