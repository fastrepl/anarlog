import { describe, expect, test } from "vitest";

import {
  createAttendanceSnapshot,
  encodeAttendanceSnapshot,
  parseAttendanceSnapshot,
} from "./attendance";

const observedAt = new Date("2026-09-30T12:00:00.000Z");
const attendance = {
  self_status: "accepted",
  roster_status: "complete",
  others: {
    accepted: 1,
    tentative: 2,
    pending: 3,
    declined: 4,
    unknown: 5,
  },
};

describe("calendar attendance snapshots", () => {
  test("creates a versioned snapshot without provider identities", () => {
    expect(createAttendanceSnapshot(attendance, observedAt)).toEqual({
      version: 1,
      ...attendance,
      observed_at: "2026-09-30T12:00:00.000Z",
    });
  });

  test.each([
    null,
    undefined,
    {},
    { ...attendance, self_status: "maybe" },
    { ...attendance, roster_status: "hidden" },
    { ...attendance, others: { ...attendance.others, declined: -1 } },
    { ...attendance, others: { ...attendance.others, declined: 1.5 } },
  ])("turns invalid provider attendance into null", (value) => {
    expect(createAttendanceSnapshot(value, observedAt)).toBeNull();
  });

  test("encodes and parses a valid snapshot", () => {
    const snapshot = createAttendanceSnapshot(attendance, observedAt);
    expect(parseAttendanceSnapshot(encodeAttendanceSnapshot(snapshot))).toEqual(
      snapshot,
    );
  });

  test.each([
    null,
    undefined,
    "",
    "not json",
    "[]",
    "{}",
    JSON.stringify({
      version: 2,
      ...attendance,
      observed_at: observedAt.toISOString(),
    }),
    JSON.stringify({
      version: 1,
      ...attendance,
      observed_at: "not-a-date",
    }),
    JSON.stringify({
      version: 1,
      ...attendance,
      others: { ...attendance.others, pending: Number.MAX_SAFE_INTEGER + 1 },
      observed_at: observedAt.toISOString(),
    }),
  ])("returns null without throwing for invalid persisted input", (value) => {
    expect(parseAttendanceSnapshot(value)).toBeNull();
  });

  test("ignores extra fields while returning a canonical snapshot", () => {
    const value = JSON.stringify({
      version: 1,
      ...attendance,
      provider_email: "private@example.com",
      others: { ...attendance.others, future_field: true },
      observed_at: observedAt.toISOString(),
    });

    expect(parseAttendanceSnapshot(value)).toEqual({
      version: 1,
      ...attendance,
      observed_at: observedAt.toISOString(),
    });
  });
});
