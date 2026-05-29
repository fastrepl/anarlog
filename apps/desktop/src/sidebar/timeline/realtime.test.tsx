import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { CurrentTimeIndicator } from "./realtime";

describe("CurrentTimeIndicator", () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  test("renders inside-item progress from bottom to top", () => {
    vi.useFakeTimers();

    vi.setSystemTime(new Date(2024, 0, 15, 12, 0, 0));

    const { container, rerender } = render(
      <CurrentTimeIndicator variant="inside" progress={0} />,
    );

    expect((container.firstChild as HTMLDivElement | null)?.style.top).toBe(
      "100%",
    );

    rerender(<CurrentTimeIndicator variant="inside" progress={1} />);

    expect((container.firstChild as HTMLDivElement | null)?.style.top).toBe(
      "0%",
    );
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
});
