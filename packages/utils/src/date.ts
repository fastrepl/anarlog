import { format as dateFnsFormat, isValid } from "date-fns";

export * from "date-fns";
export { TZDate } from "@date-fns/tz";

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
