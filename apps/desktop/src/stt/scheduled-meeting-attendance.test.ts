import { describe, expect, test } from "vitest";

import {
  ATTENDANCE_FUTURE_TOLERANCE_MS,
  ATTENDANCE_MAX_AGE_MS,
  decideAutomaticMeetingAttendance,
} from "./scheduled-meeting-attendance";

const NOW = new Date("2026-09-30T12:00:00.000Z").getTime();

function attendance({
  selfStatus = "accepted",
  rosterStatus = "complete",
  accepted = 1,
  tentative = 0,
  pending = 0,
  declined = 0,
  unknown = 0,
  observedAt = NOW,
}: {
  selfStatus?:
    | "organizer"
    | "accepted"
    | "tentative"
    | "pending"
    | "declined"
    | "unknown";
  rosterStatus?: "complete" | "incomplete" | "unknown";
  accepted?: number;
  tentative?: number;
  pending?: number;
  declined?: number;
  unknown?: number;
  observedAt?: number;
} = {}) {
  return JSON.stringify({
    version: 1,
    self_status: selfStatus,
    roster_status: rosterStatus,
    others: { accepted, tentative, pending, declined, unknown },
    observed_at: new Date(observedAt).toISOString(),
  });
}

function decide(attendanceJson: string | null, nowMs = NOW) {
  return decideAutomaticMeetingAttendance({ attendanceJson, nowMs });
}

describe("decideAutomaticMeetingAttendance", () => {
  test.each(["organizer", "accepted"] as const)(
    "allows fresh %s intent",
    (selfStatus) => {
      expect(decide(attendance({ selfStatus }))).toEqual({
        eligible: true,
        basis: selfStatus,
      });
    },
  );

  test.each([
    ["declined", "self_declined"],
    ["tentative", "self_tentative"],
    ["pending", "self_pending"],
    ["unknown", "self_unknown"],
  ] as const)("blocks %s intent", (selfStatus, reason) => {
    expect(decide(attendance({ selfStatus }))).toEqual({
      eligible: false,
      reasons: [reason],
    });
  });

  test("blocks an organizer when every other human declined", () => {
    expect(
      decide(
        attendance({
          selfStatus: "organizer",
          accepted: 0,
          declined: 3,
        }),
      ),
    ).toEqual({
      eligible: false,
      reasons: ["all_others_declined"],
    });
  });

  test("allows an accepted non-organizer when the organizer remains", () => {
    expect(decide(attendance({ accepted: 1, declined: 3 }))).toEqual({
      eligible: true,
      basis: "accepted",
    });
  });

  test("reports unanswered intent and an all-declined group together", () => {
    expect(
      decide(attendance({ selfStatus: "pending", accepted: 0, declined: 2 })),
    ).toEqual({
      eligible: false,
      reasons: ["self_pending", "all_others_declined"],
    });
  });

  test.each([
    { accepted: 1 },
    { accepted: 0, pending: 1, declined: 2 },
    { accepted: 0, tentative: 1, declined: 2 },
    { accepted: 0, unknown: 1, declined: 2 },
  ])("does not treat a mixed group as all declined: %o", (others) => {
    expect(decide(attendance(others))).toMatchObject({ eligible: true });
  });

  test("does not treat an empty roster as all declined", () => {
    expect(decide(attendance({ accepted: 0 }))).toEqual({
      eligible: true,
      basis: "accepted",
    });
  });

  test.each(["incomplete", "unknown"] as const)(
    "does not infer all declined from a %s roster",
    (rosterStatus) => {
      expect(
        decide(attendance({ rosterStatus, accepted: 0, declined: 2 })),
      ).toEqual({ eligible: true, basis: "accepted" });
    },
  );

  test("blocks missing attendance", () => {
    expect(decide(null)).toEqual({
      eligible: false,
      reasons: ["missing_attendance"],
    });
  });

  test.each(["not-json", "{}", "[]", '{"version":2}'])(
    "blocks invalid attendance: %s",
    (attendanceJson) => {
      expect(decide(attendanceJson)).toEqual({
        eligible: false,
        reasons: ["invalid_attendance"],
      });
    },
  );

  test("allows evidence exactly at the freshness boundary", () => {
    expect(
      decide(attendance({ observedAt: NOW - ATTENDANCE_MAX_AGE_MS })),
    ).toMatchObject({ eligible: true });
  });

  test("blocks evidence beyond the freshness boundary", () => {
    expect(
      decide(attendance({ observedAt: NOW - ATTENDANCE_MAX_AGE_MS - 1 })),
    ).toEqual({
      eligible: false,
      reasons: ["stale_attendance"],
    });
  });

  test("allows small forward clock skew", () => {
    expect(
      decide(attendance({ observedAt: NOW + ATTENDANCE_FUTURE_TOLERANCE_MS })),
    ).toMatchObject({ eligible: true });
  });

  test("blocks evidence too far in the future", () => {
    expect(
      decide(
        attendance({
          observedAt: NOW + ATTENDANCE_FUTURE_TOLERANCE_MS + 1,
        }),
      ),
    ).toEqual({
      eligible: false,
      reasons: ["future_attendance"],
    });
  });
});
