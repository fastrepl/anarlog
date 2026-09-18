import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MessageTimestamp } from "./timestamp";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("MessageTimestamp", () => {
  it("uses local calendar days for today and yesterday across a month boundary", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 8, 1, 0, 30));
    const today = new Date(2026, 8, 1, 0, 15);
    const yesterday = new Date(2026, 7, 31, 23, 45);
    const { container } = render(
      <>
        <MessageTimestamp createdAt={today.getTime()} showDate />
        <MessageTimestamp createdAt={yesterday.getTime()} showDate />
      </>,
    );
    const times = container.querySelectorAll("time");
    expect(times[0].textContent).toBe("today 12:15 AM");
    expect(times[1].textContent).toBe("yesterday 11:45 PM");
    expect(times[1].dateTime).toBe(yesterday.toISOString());
  });

  it("includes the year for older conversations and only time for messages", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 8, 18));
    const createdAt = new Date(2025, 2, 4, 18, 33).getTime();
    const { container } = render(
      <>
        <MessageTimestamp createdAt={createdAt} showDate />
        <MessageTimestamp createdAt={createdAt} />
      </>,
    );
    const times = container.querySelectorAll("time");
    expect(times[0].textContent).toBe("Mar 4, 2025 6:33 PM");
    expect(times[1].textContent).toBe("6:33 PM");
  });

  it("omits missing and invalid dates instead of showing a made-up time", () => {
    const { container } = render(
      <>
        <MessageTimestamp />
        <MessageTimestamp createdAt={NaN} />
        <MessageTimestamp createdAt={1e20} />
      </>,
    );
    expect(container.querySelector("time")).toBeNull();
  });
});
