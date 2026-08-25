import assert from "node:assert/strict";
import test from "node:test";

import {
  buildSessionList,
  dayLabel,
  mapTimelineRows,
  nextTimelineRefreshAt,
  relativeLabel,
} from "./timeline-model.ts";

test("uses calendar labels around the current day", () => {
  const now = new Date(2026, 7, 17, 12).getTime();

  assert.equal(dayLabel(new Date(2026, 7, 17, 23).toISOString(), now), "Today");
  assert.equal(
    dayLabel(new Date(2026, 7, 18, 0).toISOString(), now),
    "Tomorrow",
  );
  assert.equal(
    dayLabel(new Date(2026, 7, 16, 23).toISOString(), now),
    "Yesterday",
  );
});

test("uses readable relative units instead of unbounded hours", () => {
  const now = new Date("2026-08-17T12:00:00.000Z").getTime();

  assert.equal(relativeLabel(new Date(now - 29_000).toISOString(), now), "now");
  assert.equal(
    relativeLabel(new Date(now - 90 * 60_000).toISOString(), now),
    "2 hours ago",
  );
  assert.equal(
    relativeLabel(new Date(now - 205 * 60 * 60_000).toISOString(), now),
    "9 days ago",
  );
  assert.equal(
    relativeLabel(new Date(now + 2 * 24 * 60 * 60_000).toISOString(), now),
    "in 2 days",
  );
});

test("groups upcoming meetings nearest-first and past meetings newest-first", () => {
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

  assert.deepEqual(
    items.filter((item) => item.type === "group").map((item) => item.label),
    ["Upcoming", "Past"],
  );
  assert.deepEqual(
    items
      .filter((item) => item.type === "session")
      .map((item) => item.session.id),
    ["upcoming-near", "upcoming-far", "past-recent", "past-old"],
  );
});

test("omits empty timeline groups", () => {
  const now = new Date(2026, 7, 17, 12).getTime();
  const items = buildSessionList(
    [
      {
        id: "past",
        title: "Past",
        startedAt: new Date(now - 1_000).toISOString(),
      },
    ],
    now,
  );

  assert.deepEqual(
    items.filter((item) => item.type === "group").map((item) => item.label),
    ["Past"],
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
      },
      {
        id: "local",
        title: "Local",
        startedAt: "2026-08-17T02:00:00.000Z",
      },
      {
        id: "malformed-event",
        title: "Malformed event",
        startedAt: "2026-08-17T03:00:00.000Z",
      },
    ],
  );
});

test("retains a session with an invalid date in the past group", () => {
  const items = buildSessionList(
    [{ id: "invalid", title: "Invalid", startedAt: "not-a-date" }],
    Date.now(),
  );

  assert.deepEqual(
    items.filter((item) => item.type === "group").map((item) => item.label),
    ["Past"],
  );
  assert.equal(
    items.find((item) => item.type === "header")?.label,
    "Date unavailable",
  );
});

test("refreshes at the next meeting boundary before the minute tick", () => {
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
});

test("refreshes at the next minute when no meeting starts sooner", () => {
  const now = new Date("2026-08-17T12:00:30.000Z").getTime();

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
