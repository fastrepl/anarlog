import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNewMeeting: vi.fn(),
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
  useNewNoteAndListen: () => mocks.createNewMeeting,
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
    mocks.createNewMeeting.mockClear();
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
      name: /Create new meeting/,
    });

    fireEvent.pointerDown(createButton, {
      button: 0,
      clientX: 10,
      clientY: 10,
      pointerId: 1,
    });
    fireEvent.click(createButton);

    expect(mocks.startDragging).not.toHaveBeenCalled();
    expect(mocks.createNewMeeting).toHaveBeenCalledTimes(1);
  });

  it("starts window drag and ignores the release click after dragging", () => {
    render(<TopMeetingTimeline currentTab={null} />);

    const createButton = screen.getByRole("button", {
      name: /Create new meeting/,
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
    expect(mocks.createNewMeeting).not.toHaveBeenCalled();
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
    const startMetadata = screen.getByText(startLabel);
    const buttonText = title.closest("button")?.textContent ?? "";

    expect(title.closest("button")).toBe(startMetadata.closest("button"));
    expect(buttonText.indexOf("Design Review")).toBeLessThan(
      buttonText.indexOf(startLabel),
    );
    expect(startLabel).not.toContain("-");
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

    expect(cardButton?.className).toContain("bg-red-500");
    expect(cardButton?.className).not.toContain("bg-neutral-900");

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
    expect(spinnerSuffix.className).toContain("text-white/70");
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

    expect(screen.getByTestId("top-timeline-now-indicator").style.left).toBe(
      "80px",
    );
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

  it("places the current time marker between open-ended notes and future meetings", () => {
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

    expect(screen.getByTestId("top-timeline-now-indicator").style.left).toBe(
      "162px",
    );
  });

  it("places the current time marker at the edge of active ad-hoc meetings", () => {
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

    expect(screen.getByTestId("top-timeline-now-indicator").style.left).toBe(
      "160px",
    );
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
    scrollContainer!.scrollLeft = 250;
    fireEvent.scroll(scrollContainer!);

    fireEvent.click(screen.getByRole("button", { name: "Now" }));

    expect(scrollContainer!.scrollLeft).toBe(112);
  });
});
