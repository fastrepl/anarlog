import { describe, expect, test } from "vitest";

import { eventCalendarDay } from "./event-day";

describe("eventCalendarDay", () => {
  test("keeps Outlook all-day dates on the calendar day west of UTC", () => {
    expect(
      eventCalendarDay("2026-08-27T00:00:00", true, "America/Los_Angeles"),
    ).toBe("2026-08-27");
    expect(
      eventCalendarDay(
        "2026-08-27T00:00:00+00:00",
        true,
        "America/Los_Angeles",
      ),
    ).toBe("2026-08-27");
  });

  test("groups timed events by the instant in the display timezone", () => {
    expect(
      eventCalendarDay(
        "2026-08-27T02:00:00+00:00",
        false,
        "America/Los_Angeles",
      ),
    ).toBe("2026-08-26");
    expect(
      eventCalendarDay("2026-08-27T12:00:00.0000000", false, "Europe/Paris"),
    ).toBe("2026-08-27");
  });
});
