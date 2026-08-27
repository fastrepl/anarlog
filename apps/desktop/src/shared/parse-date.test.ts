import { describe, expect, test } from "vitest";

import { parseEventInstant, safeParseDate } from "@anlg/utils";

describe("safeParseDate", () => {
  test("keeps datetime-local wall-clock values in local time", () => {
    const parsed = safeParseDate("2026-08-27T14:00");
    expect(parsed?.getTime()).toBe(new Date("2026-08-27T14:00").getTime());
  });

  test("does not treat naive all-day midnights as UTC", () => {
    const parsed = safeParseDate("2026-08-27T00:00:00");
    expect(parsed?.getTime()).toBe(new Date("2026-08-27T00:00:00").getTime());
  });

  test("keeps explicit offsets", () => {
    const parsed = safeParseDate("2026-08-27T14:00:00+02:00");
    expect(parsed?.toISOString()).toBe("2026-08-27T12:00:00.000Z");
  });

  test("keeps Z-suffixed timestamps", () => {
    const parsed = safeParseDate("2026-08-27T12:00:00.000Z");
    expect(parsed?.toISOString()).toBe("2026-08-27T12:00:00.000Z");
  });
});

describe("parseEventInstant", () => {
  test("treats timezone-naive Graph timestamps as UTC", () => {
    const parsed = parseEventInstant("2026-08-27T12:00:00.0000000");
    expect(parsed?.toISOString()).toBe("2026-08-27T12:00:00.000Z");
  });

  test("keeps explicit offsets", () => {
    const parsed = parseEventInstant("2026-08-27T14:00:00+02:00");
    expect(parsed?.toISOString()).toBe("2026-08-27T12:00:00.000Z");
  });
});
