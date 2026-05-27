import { describe, expect, test } from "vitest";

import { normalizeEndMs, TIMELINE_BLOCK_MS } from "./meeting-timeline-layout";

describe("meeting timeline layout", () => {
  test("uses one 30-minute block when an item has no end time", () => {
    const start = new Date("2026-05-27T09:00:00.000Z");

    expect(normalizeEndMs(start, null)).toBe(
      start.getTime() + TIMELINE_BLOCK_MS,
    );
  });

  test("uses one 30-minute block when an item end is before its start", () => {
    const start = new Date("2026-05-27T09:00:00.000Z");
    const end = new Date("2026-05-27T08:45:00.000Z");

    expect(normalizeEndMs(start, end)).toBe(
      start.getTime() + TIMELINE_BLOCK_MS,
    );
  });

  test("keeps valid end times", () => {
    const start = new Date("2026-05-27T09:00:00.000Z");
    const end = new Date("2026-05-27T09:45:00.000Z");

    expect(normalizeEndMs(start, end)).toBe(end.getTime());
  });
});
