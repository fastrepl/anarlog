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

  if (typeof value === "number") {
    const date = new Date(value);
    return isValid(date) ? date : null;
  }

  if (typeof value === "string") {
    const date = new Date(toAbsoluteIso(value));
    return isValid(date) ? date : null;
  }

  return null;
}

function toAbsoluteIso(value: string): string {
  const trimmed = value.trim();
  // Graph/SQLite event times are UTC instants. A timezone-naive ISO string is
  // local in JS Date, which shifts CEST (and similar) notifications early.
  if (NAIVE_DATE_TIME.test(trimmed)) {
    return `${trimmed}Z`;
  }
  return trimmed;
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
