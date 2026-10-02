import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { CurrentTimeIndicator } from "./realtime";

const mocks = vi.hoisted(() => ({ use24HourTime: false }));
vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.use24HourTime,
}));

describe("CurrentTimeIndicator", () => {
  afterEach(() => {
    cleanup();
    mocks.use24HourTime = false;
    vi.useRealTimers();
  });

  test("syncs the label at the next wall-clock minute", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2024, 0, 15, 12, 0, 45));

    render(<CurrentTimeIndicator />);

    expect(screen.getByText("12:00 PM")).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(15_099);
    });

    expect(screen.getByText("12:00 PM")).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(1);
    });

    expect(screen.getByText("12:01 PM")).toBeTruthy();
  });

  test.each([
    ["2026-09-17T00:00:00Z", "12:00 AM", "00:00"],
    ["2026-09-17T12:00:00Z", "12:00 PM", "12:00"],
    ["2026-09-17T14:30:00Z", "2:30 PM", "14:30"],
  ])(
    "updates the clock format immediately at %s",
    (instant, twelve, twentyFour) => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date(instant));
      const { rerender } = render(<CurrentTimeIndicator timezone="UTC" />);
      expect(screen.getByText(twelve)).toBeTruthy();
      mocks.use24HourTime = true;
      rerender(<CurrentTimeIndicator timezone="UTC" />);
      expect(screen.getByText(twentyFour)).toBeTruthy();
      mocks.use24HourTime = false;
      rerender(<CurrentTimeIndicator timezone="UTC" />);
      expect(screen.getByText(twelve)).toBeTruthy();
    },
  );
});
