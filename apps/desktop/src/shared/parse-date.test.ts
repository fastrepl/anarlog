import { describe, expect, test } from "vitest";

import { safeParseDate } from "@anlg/utils";

describe("safeParseDate", () => {
  test("treats timezone-naive ISO datetimes as UTC", () => {
    const parsed = safeParseDate("2026-08-27T12:00:00.0000000");
    expect(parsed?.toISOString()).toBe("2026-08-27T12:00:00.000Z");
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
