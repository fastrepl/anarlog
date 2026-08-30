import { describe, expect, test } from "vitest";

import {
  applyTimelineOrder,
  remapTimelineIndicatorIndex,
  resolveTimelineOrder,
} from "./data";
import type { TimelineBucket } from "./utils";

function bucket(label: string): TimelineBucket {
  return {
    label,
    precision: "date",
    items: [
      {
        type: "session",
        id: label,
        data: { title: label },
      },
    ],
  };
}

describe("timeline order", () => {
  test("resolveTimelineOrder falls back to upcoming first", () => {
    expect(resolveTimelineOrder(undefined)).toBe("upcoming_first");
    expect(resolveTimelineOrder("")).toBe("upcoming_first");
    expect(resolveTimelineOrder("bogus")).toBe("upcoming_first");
    expect(resolveTimelineOrder("chronological")).toBe("chronological");
  });

  test("applyTimelineOrder keeps upcoming-first buckets in place", () => {
    const buckets = [bucket("Tomorrow"), bucket("Today"), bucket("Yesterday")];

    expect(applyTimelineOrder(buckets, "upcoming_first")).toEqual(buckets);
  });

  test("applyTimelineOrder puts older notes first", () => {
    const buckets = [bucket("Tomorrow"), bucket("Today"), bucket("Yesterday")];

    expect(
      applyTimelineOrder(buckets, "chronological").map((entry) => entry.label),
    ).toEqual(["Yesterday", "Today", "Tomorrow"]);
  });

  test("remapTimelineIndicatorIndex mirrors the seam after reversing buckets", () => {
    expect(remapTimelineIndicatorIndex(2, 4, "upcoming_first")).toBe(2);
    expect(remapTimelineIndicatorIndex(2, 4, "chronological")).toBe(2);
    expect(remapTimelineIndicatorIndex(0, 4, "chronological")).toBe(4);
    expect(remapTimelineIndicatorIndex(4, 4, "chronological")).toBe(0);
    expect(remapTimelineIndicatorIndex(-1, 4, "chronological")).toBe(0);
    expect(remapTimelineIndicatorIndex(1, 4, "chronological")).toBe(3);
  });
});
