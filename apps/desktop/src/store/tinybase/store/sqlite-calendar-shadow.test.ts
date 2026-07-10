import { describe, expect, it } from "vitest";

import {
  normalizeCalendarRow,
  normalizeEventRow,
} from "./sqlite-calendar-shadow";

describe("SQLite calendar row normalization", () => {
  it("preserves provider identity and enabled state", () => {
    expect(
      normalizeCalendarRow({
        user_id: "user-1",
        created_at: "created",
        tracking_id_calendar: "remote-1",
        name: "Work",
        enabled: true,
        provider: "google",
        source: "primary",
        color: "#123456",
        connection_id: "connection-1",
      }),
    ).toMatchObject({
      tracking_id_calendar: "remote-1",
      enabled: true,
      provider: "google",
      connection_id: "connection-1",
    });
  });

  it("preserves event recurrence and participants JSON", () => {
    expect(
      normalizeEventRow({
        user_id: "user-1",
        created_at: "created",
        tracking_id_event: "remote-event-1",
        calendar_id: "calendar-1",
        title: "Planning",
        started_at: "start",
        ended_at: "end",
        recurrence_series_id: "series-1",
        has_recurrence_rules: true,
        is_all_day: false,
        participants_json: '[{"email":"ada@example.com"}]',
      }),
    ).toMatchObject({
      tracking_id_event: "remote-event-1",
      recurrence_series_id: "series-1",
      has_recurrence_rules: true,
      participants_json: '[{"email":"ada@example.com"}]',
    });
  });
});
