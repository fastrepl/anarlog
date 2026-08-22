import { describe, expect, test } from "vitest";

import {
  AUTO_STOP_NETWORK_HOLD_MS,
  AUTO_STOP_RECENT_OFFLINE_MS,
  isRecentNetworkDrop,
  resolveNetworkHoldUntilMs,
} from "./auto-stop";

describe("resolveNetworkHoldUntilMs", () => {
  test("keeps a future calendar deadline", () => {
    expect(
      resolveNetworkHoldUntilMs({
        calendarDeadlineMs: 2_000,
        nowMs: 1_000,
      }),
    ).toBe(2_000);
  });

  test("falls back to the default hold when no calendar deadline exists", () => {
    expect(
      resolveNetworkHoldUntilMs({
        calendarDeadlineMs: null,
        nowMs: 1_000,
      }),
    ).toBe(1_000 + AUTO_STOP_NETWORK_HOLD_MS);
  });

  test("falls back to the default hold when the calendar deadline has passed", () => {
    expect(
      resolveNetworkHoldUntilMs({
        calendarDeadlineMs: 500,
        nowMs: 1_000,
      }),
    ).toBe(1_000 + AUTO_STOP_NETWORK_HOLD_MS);
  });
});

describe("isRecentNetworkDrop", () => {
  test("is true inside the recent-reconnect window", () => {
    expect(
      isRecentNetworkDrop(1_000, 1_000 + AUTO_STOP_RECENT_OFFLINE_MS),
    ).toBe(true);
  });

  test("is false after the recent-reconnect window", () => {
    expect(
      isRecentNetworkDrop(1_000, 1_000 + AUTO_STOP_RECENT_OFFLINE_MS + 1),
    ).toBe(false);
  });

  test("is false when no reconnect was recorded", () => {
    expect(isRecentNetworkDrop(null, 1_000)).toBe(false);
  });
});
