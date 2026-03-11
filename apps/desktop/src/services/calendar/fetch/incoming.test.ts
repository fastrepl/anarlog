import { describe, expect, test } from "vitest";

import type { CalendarEvent } from "@hypr/plugin-calendar";

import { normalizeCalendarEvent } from "./incoming";

describe("normalizeCalendarEvent", () => {
  test("dedupes the current user when they appear as organizer and attendee", async () => {
    const calendarEvent = {
      provider: "google",
      id: "event-1",
      calendar_id: "calendar-1",
      external_id: "external-1",
      title: "Jensen Hamblin <> john",
      description: null,
      location: null,
      url: null,
      meeting_link: "https://hyrnote.zoom.us/j/123",
      started_at: "2026-03-11T14:40:00.000Z",
      ended_at: "2026-03-11T15:00:00.000Z",
      timezone: "America/New_York",
      is_all_day: false,
      status: "confirmed",
      organizer: {
        name: "John Jeong",
        email: "john@char.com",
        is_current_user: true,
      },
      attendees: [
        {
          name: "John",
          email: "john@char.com",
          is_current_user: true,
          status: "accepted",
          role: "required",
        },
        {
          name: "Jensen Hamblin",
          email: "jensen.hamblin@icloud.com",
          is_current_user: false,
          status: "accepted",
          role: "required",
        },
      ],
      has_recurrence_rules: false,
      recurring_event_id: null,
      raw: "{}",
    } satisfies CalendarEvent;

    const { eventParticipants } = await normalizeCalendarEvent(calendarEvent);

    expect(eventParticipants).toEqual([
      {
        name: "John Jeong",
        email: "john@char.com",
        is_organizer: true,
        is_current_user: true,
      },
      {
        name: "Jensen Hamblin",
        email: "jensen.hamblin@icloud.com",
        is_organizer: false,
        is_current_user: false,
      },
    ]);
  });
});
