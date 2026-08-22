import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  options: null as null | {
    enabled?: boolean;
    params?: unknown[];
    sql: string;
  },
  rows: [] as Array<Record<string, unknown>>,
  loading: false,
}));

vi.mock("~/db", () => ({
  executeTransaction: vi.fn(),
  liveQueryClient: { execute: vi.fn() },
  useLiveQuery: (options: {
    enabled?: boolean;
    params?: unknown[];
    sql: string;
  }) => {
    mocks.options = options;
    return {
      data: options.enabled === false || mocks.loading ? undefined : mocks.rows,
    };
  },
}));

import { useSessionSummariesByIds } from "./sessions";

describe("session SQLite queries", () => {
  beforeEach(() => {
    mocks.options = null;
    mocks.rows = [];
    mocks.loading = false;
  });

  it("loads deduplicated summaries only for referenced ids", () => {
    mocks.rows = [
      {
        id: "session-1",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
      },
    ];

    const { result } = renderHook(() =>
      useSessionSummariesByIds(["session-1", "session-1", ""]),
    );

    expect(result.current).toEqual(mocks.rows);
    expect(mocks.options?.enabled).toBe(true);
    expect(mocks.options?.params).toEqual(["session-1"]);
    expect(mocks.options?.sql).toContain("WHERE id IN (?)");
  });

  it("does not expose summaries when no ids are referenced", () => {
    mocks.rows = [
      {
        id: "session-1",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
      },
    ];

    const { result } = renderHook(() => useSessionSummariesByIds([]));

    expect(result.current).toEqual([]);
    expect(mocks.options?.enabled).toBe(false);
    expect(mocks.options?.params).toEqual([]);
    expect(mocks.options?.sql).toContain("WHERE id IN (NULL)");
  });

  it("keeps the last resolved summaries while a by-id query is loading", () => {
    mocks.rows = [
      {
        id: "session-1",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
      },
    ];

    const { result, rerender } = renderHook(
      ({ ids }) => useSessionSummariesByIds(ids),
      { initialProps: { ids: ["session-1"] } },
    );

    expect(result.current).toEqual(mocks.rows);

    mocks.loading = true;
    rerender({ ids: ["session-1", "session-2"] });

    expect(result.current).toEqual([
      {
        id: "session-1",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
      },
    ]);
    expect(mocks.options?.params).toEqual(["session-1", "session-2"]);
  });

  it("drops held summaries when no ids are referenced", () => {
    mocks.rows = [
      {
        id: "session-1",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
      },
    ];

    const { result, rerender } = renderHook(
      ({ ids }) => useSessionSummariesByIds(ids),
      { initialProps: { ids: ["session-1"] } },
    );

    expect(result.current).toEqual(mocks.rows);

    mocks.loading = true;
    rerender({ ids: [] });

    expect(result.current).toEqual([]);
  });
});
