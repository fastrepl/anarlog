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

  test("fires onExpire when an active countdown reaches the start time", () => {
    const onExpire = vi.fn();
    useSessionEventMock.mockReturnValue({
      started_at: new Date(Date.now() + 2000).toISOString(),
    });

    const { result } = renderHook(() =>
      useEventCountdown("session-1", { onExpire }),
    );

    expect(result.current.label).toBe("starts in 2s");

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(result.current.label).toBeNull();
    expect(onExpire).toHaveBeenCalledTimes(1);
  });

  test("does not fire onExpire for an event that is already in the past", () => {
    const onExpire = vi.fn();
    useSessionEventMock.mockReturnValue({
      started_at: new Date(Date.now() - 1000).toISOString(),
    });

    renderHook(() => useEventCountdown("session-1", { onExpire }));

    expect(onExpire).not.toHaveBeenCalled();
  });
});
