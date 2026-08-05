import { beforeEach, describe, expect, test, vi } from "vitest";

const calendarCommands = vi.hoisted(() => ({
  listEvents: vi.fn(),
  parseMeetingLink: vi.fn(),
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
    calendarCommands.parseMeetingLink.mockResolvedValue(undefined);
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

  test("prefers the location meeting link over links in the description", async () => {
    const meetingLink = "https://meet.google.com/abc-defg-hij";
    calendarCommands.parseMeetingLink.mockImplementation(async (text) => text);
    calendarCommands.listEvents.mockResolvedValue({
      status: "success",
      data: [
        {
          id: "event-1",
          calendar_id: "primary",
          title: "Customer call",
          description: "https://cal.com/customer-call/reschedule",
          location: meetingLink,
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
    expect(calendarCommands.parseMeetingLink).toHaveBeenCalledOnce();
    expect(calendarCommands.parseMeetingLink).toHaveBeenCalledWith(meetingLink);
  });
});
