import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { useEventCountdown } from "./useEventCountdown";

const { useSessionEventMock } = vi.hoisted(() => ({
  useSessionEventMock: vi.fn(),
}));

vi.mock("~/session/hooks/useSessionEvent", () => ({
  useSessionEvent: useSessionEventMock,
}));

describe("useEventCountdown", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-05-15T12:00:00.000Z"));
    useSessionEventMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("clears the label when an active countdown reaches the start time", () => {
    useSessionEventMock.mockReturnValue({
      started_at: new Date(Date.now() + 2000).toISOString(),
    });

    const { result } = renderHook(() => useEventCountdown("session-1"));

    expect(result.current.label).toBe("starts in 2s");

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(result.current.label).toBeNull();
  });

  test("shows no label for an event that is already in the past", () => {
    useSessionEventMock.mockReturnValue({
      started_at: new Date(Date.now() - 1000).toISOString(),
    });

    const { result } = renderHook(() => useEventCountdown("session-1"));

    expect(result.current.label).toBeNull();
  });

  test("shows no label until the event is within five minutes", () => {
    useSessionEventMock.mockReturnValue({
      started_at: new Date(Date.now() + 6 * 60_000).toISOString(),
    });

    const { result } = renderHook(() => useEventCountdown("session-1"));

    expect(result.current.label).toBeNull();

    act(() => {
      vi.advanceTimersByTime(61_000);
    });

    expect(result.current.label).toBe("starts in 4m 59s");
  });
});
