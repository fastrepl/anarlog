import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  updateIgnoredCalendarItem: vi.fn(),
  rowsById: {} as Record<string, Array<Record<string, unknown>>>,
}));

vi.mock("@anlg/plugin-calendar", () => ({
  commands: {
    updateIgnoredCalendarItem: mocks.updateIgnoredCalendarItem,
  },
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: vi.fn() },
  useLiveQuery: (options: {
    params: unknown[];
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => ({
    data: options.mapRows(mocks.rowsById[String(options.params[0])] ?? []),
  }),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import { useIgnoredEvents } from "./ignored-events";

describe("SQLite ignored events", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.rowsById = {};
  });

  it("reads ignored events and recurring series from SQLite", () => {
    mocks.rowsById.ignored_events = [
      {
        value_json: JSON.stringify([
          { tracking_id: "event-1", last_seen: "2026-07-10T00:00:00.000Z" },
        ]),
      },
    ];
    mocks.rowsById.ignored_recurring_series = [
      {
        value_json: JSON.stringify(
          JSON.stringify([
            { id: "series-1", last_seen: "2026-07-10T00:00:00.000Z" },
          ]),
        ),
      },
    ];

    const { result } = renderHook(() => useIgnoredEvents());

    expect(result.current.isIgnored("event-1", null)).toBe(true);
    expect(result.current.isIgnored("event-2", "series-1")).toBe(true);
    expect(result.current.isIgnored("event-2", "series-2")).toBe(false);
    expect(result.current.isIgnored(null, "series-1")).toBe(false);
  });
});
