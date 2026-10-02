import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { useCalendarData } from "./hooks";

import type { TimelineEventRow } from "~/sidebar/timeline/utils";

const mocks = vi.hoisted(() => ({
  events: {} as Record<string, TimelineEventRow>,
  isIgnored: (trackingId: string | null | undefined) =>
    trackingId === "ignored",
}));

vi.mock("./queries", () => ({
  useTimelineTables: () => ({
    timelineEventsTable: mocks.events,
    timelineSessionsTable: {},
  }),
}));
vi.mock("./ignored-events", () => ({
  useIgnoredEvents: () => ({ isIgnored: mocks.isIgnored }),
}));
vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "timezone" ? "America/New_York" : false,
}));

const busyEvent: TimelineEventRow = {
  title: "",
  started_at: "2026-09-14T16:00:00-04:00",
  ended_at: "2026-09-14T16:30:00-04:00",
  tracking_id_event: "busy",
  calendar_id: "shared-calendar",
  description: "",
  meeting_link: "",
  recurrence_series_id: "",
  has_recurrence_rules: false,
  is_all_day: false,
};

afterEach(cleanup);

describe("untitled calendar events", () => {
  test("groups untitled events by date without bypassing ignored or invalid-date filters", () => {
    mocks.events = {
      busy: busyEvent,
      titled: { ...busyEvent, title: "Planning" },
      ignored: { ...busyEvent, tracking_id_event: "ignored" },
      invalid: { ...busyEvent, started_at: "" },
    };

    const { result } = renderHook(() => useCalendarData());

    expect(result.current.eventIdsByDate).toEqual({
      "2026-09-14": ["busy", "titled"],
    });
    expect(result.current.eventsById.busy.title).toBe("");
  });
});
