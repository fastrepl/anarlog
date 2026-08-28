import { format, parseEventInstant, TZDate } from "@anlg/utils";

const CALENDAR_DATE_KEY = /^\d{4}-\d{2}-\d{2}$/;

function toTz(date: Date, tz?: string): Date {
  return tz ? new TZDate(date, tz) : date;
}

export function eventCalendarDay(
  startedAt: string | null | undefined,
  isAllDay: boolean | number | null | undefined,
  tz?: string,
): string | null {
  if (!startedAt) {
    return null;
  }

  if (Boolean(isAllDay)) {
    const date = startedAt.trim().slice(0, 10);
    return CALENDAR_DATE_KEY.test(date) ? date : null;
  }

  const raw = parseEventInstant(startedAt);
  if (!raw) {
    return null;
  }
  return format(toTz(raw, tz), "yyyy-MM-dd");
}
