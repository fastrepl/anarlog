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

  test("uses red current-time colors in light and dark mode", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2024, 0, 15, 12, 0, 0));

    const { container } = render(<CurrentTimeIndicator />);
    const line = container.querySelector("[data-sidebar-current-time-line]");
    const label = container.querySelector("[data-sidebar-current-time-label]");

    expect(line?.className).toContain("bg-red-500/85");
    expect(line?.className).toContain("dark:bg-red-400/70");
    expect(label?.className).toContain("border-red-500");
    expect(label?.className).toContain("bg-red-500");
    expect(label?.className).toContain("text-white");
    expect(label?.className).toContain("dark:border-red-500");
    expect(label?.className).toContain("dark:bg-red-500");
    expect(label?.className).toContain("dark:text-white");
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
