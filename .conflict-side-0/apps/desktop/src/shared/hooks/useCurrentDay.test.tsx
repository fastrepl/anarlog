import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { useCurrentDay } from "./useCurrentDay";

afterEach(() => {
  vi.useRealTimers();
});

it("refreshes the current day when the window becomes visible", () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-07-17T23:59:00.000Z"));
  const view = renderHook(() => useCurrentDay("UTC"));
  const initialDay = view.result.current;

  act(() => {
    vi.setSystemTime(new Date("2026-07-18T00:01:00.000Z"));
    document.dispatchEvent(new Event("visibilitychange"));
  });

  expect(view.result.current).not.toBe(initialDay);
  view.unmount();
});
