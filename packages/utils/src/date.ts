import { format as dateFnsFormat, isValid } from "date-fns";

export * from "date-fns";
export { TZDate } from "@date-fns/tz";

const NAIVE_DATE_TIME =
  /^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?$/;

export function safeParseDate(value: unknown): Date | null {
  if (value === null || value === undefined) {
    return null;
  }

  if (value instanceof Date) {
    return isValid(value) ? value : null;
  }

  if (typeof value === "string" || typeof value === "number") {
    const date = new Date(value);
    return isValid(date) ? date : null;
  }

  return null;
}

// Timed calendar events: leftover Graph strings are UTC wall-clock.
// Do not use for datetime-local values or all-day calendar dates.
export function parseEventInstant(value: unknown): Date | null {
  if (typeof value !== "string") {
    return safeParseDate(value);
  }

  const trimmed = value.trim();
  if (NAIVE_DATE_TIME.test(trimmed)) {
    const date = new Date(`${trimmed}Z`);
    return isValid(date) ? date : null;
  }

  return safeParseDate(trimmed);
}

export function safeFormat(
  value: unknown,
  formatString: string,
  fallback = "",
): string {
  const date = safeParseDate(value);
  if (!date) {
    return fallback;
  }
  try {
    return dateFnsFormat(date, formatString);
  } catch {
    return fallback;
  }
}
