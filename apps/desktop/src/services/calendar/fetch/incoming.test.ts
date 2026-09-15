import { beforeEach, describe, expect, test, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  listEvents: vi.fn(),
}));

vi.mock("@anlg/plugin-calendar", () => ({
  commands: calendarCommands,
}));

import type { Ctx } from "../ctx";
import { fetchIncomingEvents } from "./incoming";

const ctx: Ctx = {
  provider: "google",
  connectionId: "conn-1",
  from: new Date("2026-06-01T00:00:00.000Z"),
  to: new Date("2026-06-02T00:00:00.000Z"),
  calendarIds: new Set(["cal-1"]),
  calendarTrackingIdToId: new Map([["primary", "cal-1"]]),
};

describe("fetchIncomingEvents", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  test("records an empty participant list so stale auto mappings are removed", async () => {
    calendarCommands.listEvents.mockResolvedValue({
      status: "success",
      data: [
        {
          id: "event-1",
          calendar_id: "primary",
          title: "No attendees",
          started_at: "2026-06-01T10:00:00.000Z",
          ended_at: "2026-06-01T11:00:00.000Z",
          attendees: [],
          organizer: null,
          has_recurrence_rules: false,
          is_all_day: false,
        },
      ],
    });

    const result = await fetchIncomingEvents(ctx);

    expect(result.events).toHaveLength(1);
    expect(result.participants.has("event-1")).toBe(true);
    expect(result.participants.get("event-1")).toEqual([]);
  });

  test("passes through the meeting link resolved during provider conversion", async () => {
    const meetingLink = "https://meet.google.com/abc-defg-hij";
    calendarCommands.listEvents.mockResolvedValue({
      status: "success",
      data: [
        {
          id: "event-1",
          calendar_id: "primary",
          title: "Customer call",
          description: "https://cal.com/customer-call/reschedule",
          location: "Conference room 4",
          meeting_link: meetingLink,
          started_at: "2026-06-01T10:00:00.000Z",
          ended_at: "2026-06-01T11:00:00.000Z",
          attendees: [],
          organizer: null,
          has_recurrence_rules: false,
          is_all_day: false,
        },
      ],
    });

    const result = await fetchIncomingEvents(ctx);

    expect(result.events[0]?.meeting_link).toBe(meetingLink);
  });

  test("excludes cancelled events before SQLite sync", async () => {
    calendarCommands.listEvents.mockResolvedValue({
      status: "success",
      data: [
        {
          id: "cancelled-standalone-event",
          calendar_id: "primary",
          title: "Cancelled one-off",
          started_at: "2026-06-01T09:00:00.000Z",
          ended_at: "2026-06-01T09:30:00.000Z",
          status: "cancelled",
          attendees: [],
          organizer: null,
          has_recurrence_rules: false,
          is_all_day: false,
        },
        {
          id: "cancelled-recurring-event",
          calendar_id: "primary",
          title: "Cancelled planning",
          started_at: "2026-06-01T10:00:00.000Z",
          ended_at: "2026-06-01T11:00:00.000Z",
          status: "cancelled",
          attendees: [],
          organizer: null,
          has_recurrence_rules: true,
          is_all_day: false,
        },
        {
          id: "confirmed-event",
          calendar_id: "primary",
          title: "Confirmed planning",
          started_at: "2026-06-01T12:00:00.000Z",
          ended_at: "2026-06-01T13:00:00.000Z",
          status: "confirmed",
          attendees: [],
          organizer: null,
          has_recurrence_rules: false,
          is_all_day: false,
        },
      ],
    });

    const result = await fetchIncomingEvents(ctx);

    expect(result.events.map((event) => event.tracking_id_event)).toEqual([
      "confirmed-event",
    ]);
    expect(result.participants.has("cancelled-standalone-event")).toBe(false);
    expect(result.participants.has("cancelled-recurring-event")).toBe(false);
  });

  test("normalizes the provider modification time", async () => {
    calendarCommands.listEvents.mockResolvedValue({
      status: "success",
      data: [
        {
          provider: "apple",
          id: "external-1:2026-09-14",
          calendar_id: "primary",
          external_id: "external-1",
          title: "Detached occurrence",
          started_at: "2026-09-16T05:00:00.000Z",
          ended_at: "2026-09-16T06:00:00.000Z",
          timezone: "America/Los_Angeles",
          provider_modified_at: "2026-09-14T12:00:00Z",
          status: "confirmed",
          attendees: [],
          organizer: null,
          has_recurrence_rules: false,
          is_all_day: false,
        },
      ],
    });

    const result = await fetchIncomingEvents({ ...ctx, provider: "apple" });

    expect(result.events[0]).toMatchObject({
      provider_modified_at: "2026-09-14T12:00:00Z",
    });
  });
});
