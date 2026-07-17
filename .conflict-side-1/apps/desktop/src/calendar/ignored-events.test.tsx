import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  rowsById: {} as Record<string, Array<Record<string, unknown>>>,
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
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

  it("promotes the imported settings document on the first mutation", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          ignored_events: JSON.stringify([
            {
              tracking_id: "event-existing",
              last_seen: "2026-07-09T00:00:00.000Z",
            },
          ]),
        }),
      },
    ]);

    const { result } = renderHook(() => useIgnoredEvents());
    act(() => result.current.ignoreEvent("event-new"));

    await waitFor(() => expect(mocks.executeTransaction).toHaveBeenCalled());

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("INSERT INTO app_settings");
    expect(statement.params[0]).toBe("ignored_events");
    expect(JSON.parse(String(statement.params[1]))).toEqual([
      {
        tracking_id: "event-existing",
        last_seen: "2026-07-09T00:00:00.000Z",
      },
      { tracking_id: "event-new", last_seen: expect.any(String) },
    ]);
  });

  it("promotes ignored events recovered from the legacy main values", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "legacy_main_values_document",
        value_json: JSON.stringify({
          ignored_events: JSON.stringify([
            { tracking_id: "event-main", last_seen: "legacy" },
          ]),
        }),
      },
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          ignored_events: JSON.stringify([
            { tracking_id: "event-settings", last_seen: "older" },
          ]),
        }),
      },
    ]);

    const { result } = renderHook(() => useIgnoredEvents());
    act(() => result.current.ignoreEvent("event-new"));

    await waitFor(() => expect(mocks.executeTransaction).toHaveBeenCalled());

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(JSON.parse(String(statement.params[1]))).toEqual([
      { tracking_id: "event-main", last_seen: "legacy" },
      { tracking_id: "event-new", last_seen: expect.any(String) },
    ]);
  });

  it("retries an optimistic update without dropping concurrent entries", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          id: "ignored_events",
          value_json: JSON.stringify([
            { tracking_id: "event-1", last_seen: "first" },
          ]),
        },
      ])
      .mockResolvedValueOnce([
        {
          id: "ignored_events",
          value_json: JSON.stringify([
            { tracking_id: "event-1", last_seen: "first" },
            { tracking_id: "event-concurrent", last_seen: "concurrent" },
          ]),
        },
      ]);
    mocks.executeTransaction
      .mockResolvedValueOnce([0])
      .mockResolvedValueOnce([1]);

    const { result } = renderHook(() => useIgnoredEvents());
    act(() => result.current.ignoreEvent("event-new"));

    await waitFor(() =>
      expect(mocks.executeTransaction).toHaveBeenCalledTimes(2),
    );

    const statement = mocks.executeTransaction.mock.calls[1][0][0];
    expect(JSON.parse(String(statement.params[0]))).toEqual([
      { tracking_id: "event-1", last_seen: "first" },
      { tracking_id: "event-concurrent", last_seen: "concurrent" },
      { tracking_id: "event-new", last_seen: expect.any(String) },
    ]);
  });
});
