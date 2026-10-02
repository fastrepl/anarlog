import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSessionList,
  mapTimelineRows,
  nextTimelineRefreshAt,
} from "./timeline-model.ts";

test("orders meetings without generic past or upcoming headers", () => {
  const now = new Date(2026, 7, 17, 12).getTime();
  const iso = (offsetHours) =>
    new Date(now + offsetHours * 60 * 60 * 1000).toISOString();
  const items = buildSessionList(
    [
      { id: "past-old", title: "Past old", startedAt: iso(-24) },
      { id: "upcoming-far", title: "Upcoming far", startedAt: iso(24) },
      { id: "past-recent", title: "Past recent", startedAt: iso(-1) },
      { id: "upcoming-near", title: "Upcoming near", startedAt: iso(1) },
    ],
    now,
  );

  assert.equal(
    items.some((item) => item.type === "group"),
    false,
  );
  assert.deepEqual(
    items.filter((item) => item.type === "header").map((item) => item.label),
    ["Today", "Tomorrow", "Today", "Yesterday"],
  );
  assert.deepEqual(
    items
      .filter((item) => item.type === "session")
      .map((item) => item.session.id),
    ["upcoming-near", "upcoming-far", "past-recent", "past-old"],
  );
});

test("prefers canonical event start time and falls back to creation time", () => {
  assert.deepEqual(
    mapTimelineRows([
      {
        id: "scheduled",
        title: "Scheduled",
        created_at: "2026-08-17T00:00:00.000Z",
        event_json: JSON.stringify({ started_at: "2026-08-18T01:00:00.000Z" }),
        folder_path: "Work/Planning",
        tags_json: '["roadmap","planning","roadmap"]',
      },
      {
        id: "local",
        title: "Local",
        created_at: "2026-08-17T02:00:00.000Z",
        event_json: "not-json",
      },
      {
        id: "malformed-event",
        title: "Malformed event",
        created_at: "2026-08-17T03:00:00.000Z",
        event_json: JSON.stringify({ started_at: "not-a-date" }),
      },
    ]),
    [
      {
        id: "scheduled",
        title: "Scheduled",
        startedAt: "2026-08-18T01:00:00.000Z",
        folderPath: "Work/Planning",
        tags: ["planning", "roadmap"],
      },
      {
        id: "local",
        title: "Local",
        startedAt: "2026-08-17T02:00:00.000Z",
        folderPath: "",
        tags: [],
      },
      {
        id: "malformed-event",
        title: "Malformed event",
        startedAt: "2026-08-17T03:00:00.000Z",
        folderPath: "",
        tags: [],
      },
    ],
  );
});

test("retains a session with an invalid date in the timeline", () => {
  const items = buildSessionList(
    [{ id: "invalid", title: "Invalid", startedAt: "not-a-date" }],
    Date.now(),
  );

  assert.equal(
    items.find((item) => item.type === "header")?.label,
    "Date unavailable",
  );
});

test("refreshes at the next meeting boundary or minute tick", () => {
  const now = new Date("2026-08-17T12:00:30.000Z").getTime();
  const meeting = new Date(now + 5_000).toISOString();

  assert.equal(
    nextTimelineRefreshAt(
      [
        { id: "past", title: "Past", startedAt: "2026-08-17T11:00:00.000Z" },
        { id: "invalid", title: "Invalid", startedAt: "not-a-date" },
        { id: "next", title: "Next", startedAt: meeting },
      ],
      now,
    ),
    new Date(meeting).getTime() + 1,
  );
  assert.equal(
    nextTimelineRefreshAt(
      [
        {
          id: "later",
          title: "Later",
          startedAt: "2026-08-17T12:05:00.000Z",
        },
      ],
      now,
    ),
    new Date("2026-08-17T12:01:00.001Z").getTime(),
  );
});
