import { parseAttendanceSnapshot } from "~/services/calendar/attendance";

export const ATTENDANCE_MAX_AGE_MS = 3 * 60_000;
export const ATTENDANCE_FUTURE_TOLERANCE_MS = 30_000;

export type AutomaticMeetingBlockReason =
  | "missing_attendance"
  | "invalid_attendance"
  | "stale_attendance"
  | "future_attendance"
  | "self_declined"
  | "self_tentative"
  | "self_pending"
  | "self_unknown"
  | "all_others_declined";

export type AutomaticMeetingDecision =
  | { eligible: true; basis: "organizer" | "accepted" }
  | { eligible: false; reasons: AutomaticMeetingBlockReason[] };

export function decideAutomaticMeetingAttendance({
  attendanceJson,
  nowMs,
}: {
  attendanceJson: string | null;
  nowMs: number;
}): AutomaticMeetingDecision {
  const attendance = parseAttendanceSnapshot(attendanceJson);
  if (!attendance) {
    return {
      eligible: false,
      reasons: [attendanceJson ? "invalid_attendance" : "missing_attendance"],
    };
  }

  const observedAtMs = Date.parse(attendance.observed_at);
  if (!Number.isFinite(observedAtMs)) {
    return { eligible: false, reasons: ["invalid_attendance"] };
  }
  if (observedAtMs - nowMs > ATTENDANCE_FUTURE_TOLERANCE_MS) {
    return { eligible: false, reasons: ["future_attendance"] };
  }
  if (nowMs - observedAtMs > ATTENDANCE_MAX_AGE_MS) {
    return { eligible: false, reasons: ["stale_attendance"] };
  }

  const reasons: AutomaticMeetingBlockReason[] = [];
  let basis: "organizer" | "accepted" | null = null;
  switch (attendance.self_status) {
    case "organizer":
    case "accepted":
      basis = attendance.self_status;
      break;
    case "declined":
      reasons.push("self_declined");
      break;
    case "tentative":
      reasons.push("self_tentative");
      break;
    case "pending":
      reasons.push("self_pending");
      break;
    case "unknown":
      reasons.push("self_unknown");
      break;
  }

  const others = attendance.others;
  const otherCount =
    others.accepted +
    others.tentative +
    others.pending +
    others.declined +
    others.unknown;
  if (
    attendance.roster_status === "complete" &&
    otherCount > 0 &&
    others.declined === otherCount
  ) {
    reasons.push("all_others_declined");
  }

  if (reasons.length > 0 || !basis) {
    return { eligible: false, reasons };
  }

  return { eligible: true, basis };
}
