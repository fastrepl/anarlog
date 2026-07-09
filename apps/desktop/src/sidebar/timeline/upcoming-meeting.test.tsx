import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTimeMs: Date.now(),
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

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    isIgnored: mocks.isIgnored,
  }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  QUERIES: {
    timelineEvents: "timelineEvents",
    timelineSessions: "timelineSessions",
  },
  STORE_ID: "main",
  UI: {
    useResultTable: (query: string) =>
      query === "timelineEvents"
        ? mocks.timelineEventsTable
        : mocks.timelineSessionsTable,
  },
}));

vi.mock("./realtime", () => ({
  useCurrentTimeMs: () => mocks.currentTimeMs,
}));

import { useSidebarUpcomingMeetingStatus } from "./upcoming-meeting";

describe("useSidebarUpcomingMeetingStatus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));
    mocks.currentTimeMs = Date.parse("2024-01-15T12:00:00.000Z");
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

    mocks.currentTimeMs = Date.parse("2024-01-15T12:30:01.000Z");
    active.rerender();

    expect(active.result.current).toBeNull();
  });
});
