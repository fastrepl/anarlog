import { describe, expect, it } from "vitest";

import { GOOGLE_CALENDAR_DATA_SQL } from "../google-calendar-data";
import { getGoogleCalendarDataState } from "./useGoogleCalendarDataState";

describe("Google Calendar data state", () => {
  it("keeps tombstoned local data inside the boundary", () => {
    expect(GOOGLE_CALENDAR_DATA_SQL).not.toContain("deleted_at");
    expect(GOOGLE_CALENDAR_DATA_SQL).toContain("FROM calendars");
    expect(GOOGLE_CALENDAR_DATA_SQL).toContain("FROM events");
    expect(GOOGLE_CALENDAR_DATA_SQL).toContain("FROM sessions");
  });

  it("maps a conclusive query result", () => {
    expect(
      getGoogleCalendarDataState({
        data: true,
        isLoading: false,
        error: null,
      }),
    ).toBe("present");
    expect(
      getGoogleCalendarDataState({
        data: false,
        isLoading: false,
        error: null,
      }),
    ).toBe("absent");
  });

  it("fails closed on loading and errors", () => {
    expect(
      getGoogleCalendarDataState({
        data: undefined,
        isLoading: true,
        error: null,
      }),
    ).toBe("loading");
    expect(
      getGoogleCalendarDataState({
        data: false,
        isLoading: false,
        error: new Error("database unavailable"),
      }),
    ).toBe("error");
  });
});
