import { render } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { CurrentTimeIndicator } from "./realtime";

describe("CurrentTimeIndicator", () => {
  test("renders inside-item progress from bottom to top", () => {
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date("2024-01-15T12:00:00.000Z"));

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
    } finally {
      vi.useRealTimers();
    }
  });
});
