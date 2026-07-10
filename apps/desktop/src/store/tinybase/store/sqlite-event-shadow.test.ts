import { describe, expect, it } from "vitest";

import { normalizeEventRow } from "./sqlite-event-shadow";

describe("SQLite event row normalization", () => {
  it("preserves recurrence and participants JSON", () => {
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
