import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  isIgnored: vi.fn(() => false),
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
      return input.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    }

    if (typeof input === "string") {
      return input;
    }

    const descriptor = input as {
      message?: string;
      values?: Record<string, unknown>;
    };

    return (descriptor.message ?? "").replace(
      /\{(\w+)\}/g,
      (_match: string, key: string) =>
        String(descriptor.values?.[key] ?? `{${key}}`),
    );
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@lingui/react", () => ({
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => undefined,
}));

vi.mock("~/calendar/queries", () => ({
  useTimelineTables: () => ({
    timelineEventsTable: mocks.timelineEventsTable,
    timelineSessionsTable: mocks.timelineSessionsTable,
  }),
}));

vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    isIgnored: mocks.isIgnored,
  }),
}));

import { useSidebarUpcomingMeetingStatus } from "./upcoming-meeting";

describe("useSidebarUpcomingMeetingStatus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.isIgnored.mockReturnValue(false);
    mocks.timelineEventsTable = {};
    mocks.timelineSessionsTable = {};
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("uses the deleted-event visibility option when deriving the badge item", () => {
    mocks.isIgnored.mockReturnValue(true);
    mocks.timelineEventsTable = {
      standup: {
        title: "Deleted standup",
        started_at: "2024-01-15T12:03:00.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const hidden = renderHook(() =>
      useSidebarUpcomingMeetingStatus({ showIgnored: false }),
    );

    expect(hidden.result.current).toBeNull();

    const visible = renderHook(() =>
      useSidebarUpcomingMeetingStatus({ showIgnored: true }),
    );

    expect(visible.result.current).toMatchObject({
      itemKey: "event-standup",
      label: "In 3m 0s",
      progress: 0.6,
      title: "Deleted standup",
    });
  });

  it("keeps the meeting status active until the scheduled end time", () => {
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T11:55:00.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const active = renderHook(() => useSidebarUpcomingMeetingStatus());

    expect(active.result.current).toMatchObject({
      itemKey: "event-standup",
      label: "Now",
      progress: 1,
      title: "Team standup",
    });

    vi.setSystemTime(new Date("2024-01-15T12:30:01.000Z"));
    act(() => {
      vi.advanceTimersByTime(1_000);
    });

    expect(active.result.current).toBeNull();
  });

  it("does not rerender every second while the active status is unchanged", () => {
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T11:55:00.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };
    let renderCount = 0;

    renderHook(() => {
      renderCount += 1;
      return useSidebarUpcomingMeetingStatus();
    });
    const initialRenderCount = renderCount;

    act(() => {
      vi.advanceTimersByTime(5_000);
    });

    expect(renderCount).toBe(initialRenderCount);
  });

  it("refreshes the status when the window regains focus", () => {
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T11:55:00.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const active = renderHook(() => useSidebarUpcomingMeetingStatus());
    vi.setSystemTime(new Date("2024-01-15T12:30:01.000Z"));

    act(() => window.dispatchEvent(new Event("focus")));

    expect(active.result.current).toBeNull();
  });

  it("refreshes the status when the document becomes visible", () => {
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-15T11:55:00.000Z",
        ended_at: "2024-01-15T12:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const active = renderHook(() => useSidebarUpcomingMeetingStatus());
    vi.setSystemTime(new Date("2024-01-15T12:30:01.000Z"));

    act(() => document.dispatchEvent(new Event("visibilitychange")));

    expect(active.result.current).toBeNull();
  });

  it("rebuilds timeline buckets after a day boundary", () => {
    vi.setSystemTime(new Date("2024-01-15T23:59:00.000Z"));
    mocks.timelineEventsTable = {
      standup: {
        title: "Team standup",
        started_at: "2024-01-17T00:01:00.000Z",
        ended_at: "2024-01-17T00:30:00.000Z",
        tracking_id_event: "event-standup",
        has_recurrence_rules: false,
      },
    };

    const upcoming = renderHook(() => useSidebarUpcomingMeetingStatus());
    expect(upcoming.result.current).toBeNull();

    vi.setSystemTime(new Date("2024-01-16T23:59:00.000Z"));
    act(() => window.dispatchEvent(new Event("focus")));

    expect(upcoming.result.current).toMatchObject({
      itemKey: "event-standup",
      label: "In 2m 0s",
      title: "Team standup",
    });
  });
});
