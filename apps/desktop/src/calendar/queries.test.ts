import { describe, expect, it } from "vitest";

import {
  mapTimelineEventRows,
  mapTimelineSessionRows,
  parseEventParticipants,
} from "./queries";

describe("calendar SQLite queries", () => {
  it("keys timeline events by ID and normalizes SQLite booleans", () => {
    expect(
      mapTimelineEventRows([
        {
          id: "event-1",
          title: "Planning",
          started_at: "2026-07-10T09:00:00.000Z",
          ended_at: "2026-07-10T10:00:00.000Z",
          calendar_id: "calendar-1",
          tracking_id_event: "external-event-1",
          has_recurrence_rules: 0,
          recurrence_series_id: "",
          is_all_day: 1,
          location: "Room 1",
          meeting_link: "https://meet.example.com/planning",
          description: "Weekly plan",
          calendar_color: "#4285f4",
        },
      ]),
    ).toEqual({
      "event-1": {
        title: "Planning",
        started_at: "2026-07-10T09:00:00.000Z",
        ended_at: "2026-07-10T10:00:00.000Z",
        calendar_id: "calendar-1",
        tracking_id_event: "external-event-1",
        has_recurrence_rules: false,
        recurrence_series_id: "",
        is_all_day: true,
        location: "Room 1",
        meeting_link: "https://meet.example.com/planning",
        description: "Weekly plan",
        calendar_color: "#4285f4",
      },
    });
  });

  it("keys SQLite sessions by ID without dropping timeline fields", () => {
    expect(
      mapTimelineSessionRows([
        {
          id: "session-1",
          title: "Imported meeting",
          created_at: "2026-07-10T09:00:00.000Z",
          event_json: "",
          folder_id: "sessions/2026-07-10/session-1",
        },
      ]),
    ).toEqual({
      "session-1": {
        title: "Imported meeting",
        created_at: "2026-07-10T09:00:00.000Z",
        event_json: "",
        folder_id: "sessions/2026-07-10/session-1",
      },
    });
  });

  it("parses attached event participants without trusting malformed JSON", () => {
    expect(
      parseEventParticipants(
        JSON.stringify([{ name: "Alice", email: "alice@example.com" }]),
      ),
    ).toEqual([{ name: "Alice", email: "alice@example.com" }]);
    expect(parseEventParticipants("not-json")).toEqual([]);
    expect(parseEventParticipants("{}")).toEqual([]);
  });
});
