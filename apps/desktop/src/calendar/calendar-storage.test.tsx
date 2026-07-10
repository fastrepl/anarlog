import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(),
  liveRows: [] as Array<Record<string, unknown>>,
  liveQueryOptions: null as null | {
    sql: string;
    params?: unknown[];
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  useLiveQuery: (options: {
    sql: string;
    params?: unknown[];
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => {
    mocks.liveQueryOptions = options;
    return { data: options.mapRows(mocks.liveRows) };
  },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (
    _key: string,
    write: () => Promise<unknown>,
  ): Promise<unknown> => write(),
}));

import { setCalendarEnabled, useCalendarRows } from "./queries";

describe("calendar SQLite selection", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.liveRows = [];
    mocks.liveQueryOptions = null;
    mocks.executeTransaction.mockResolvedValue([]);
  });

  test("reads provider calendars from the canonical table", () => {
    mocks.liveRows = [
      {
        id: "calendar-1",
        tracking_id_calendar: "primary",
        name: "Work",
        enabled: 1,
        provider: "google",
        source: "work@example.com",
        color: "#4285f4",
        connection_id: "connection-1",
        created_at: "2026-07-10T00:00:00.000Z",
      },
    ];

    const { result } = renderHook(() => useCalendarRows("google"));

    expect(mocks.liveQueryOptions?.params).toEqual(["google", "google"]);
    expect(result.current).toEqual([
      {
        ...mocks.liveRows[0],
        enabled: true,
      },
    ]);
  });

  test("disabling a calendar tombstones its events in the same transaction", async () => {
    await setCalendarEnabled("calendar-1", false);

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE calendars");
    expect(statements[0].params[0]).toBe(0);
    expect(statements[1].sql).toContain("UPDATE events");
    expect(statements[1].sql).toContain("deleted_at");
    expect(statements[1].params).toContain("calendar-1");
  });
});
