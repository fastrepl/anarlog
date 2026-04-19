import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const calendarMocks = vi.hoisted(() => ({
  setCalendarEnabled: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    setCalendarEnabled: calendarMocks.setCalendarEnabled,
  },
}));

vi.mock("@hypr/utils", () => ({
  safeParseDate: vi.fn(),
  TZDate: class {},
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: vi.fn(),
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({ isIgnored: () => false }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  UI: {
    useRow: vi.fn(),
    useResultTable: vi.fn(),
  },
  QUERIES: { enabledCalendars: "enabledCalendars" },
  STORE_ID: "main",
}));

import { useSetCalendarEnabled } from "./hooks";

describe("useSetCalendarEnabled", () => {
  beforeEach(() => {
    calendarMocks.setCalendarEnabled.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  test("invokes the Rust set_calendar_enabled command with the given args", async () => {
    calendarMocks.setCalendarEnabled.mockResolvedValue({
      status: "ok",
      data: null,
    });

    const { result } = renderHook(() => useSetCalendarEnabled());

    await act(async () => {
      await result.current("cal-123", true);
    });

    expect(calendarMocks.setCalendarEnabled).toHaveBeenCalledTimes(1);
    expect(calendarMocks.setCalendarEnabled).toHaveBeenCalledWith(
      "cal-123",
      true,
    );
  });

  test("throws when the plugin returns a structured error", async () => {
    calendarMocks.setCalendarEnabled.mockResolvedValue({
      status: "error",
      error: "calendar sync store error: write failed",
    });

    const { result } = renderHook(() => useSetCalendarEnabled());

    await expect(result.current("cal-123", false)).rejects.toThrow(
      /calendar sync store error/,
    );
  });

  test("is stable across re-renders so callers can safely memoize on it", () => {
    calendarMocks.setCalendarEnabled.mockResolvedValue({
      status: "ok",
      data: null,
    });

    const { result, rerender } = renderHook(() => useSetCalendarEnabled());
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
  });
});
