import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  rows: [] as Array<{ status: string }>,
  liveQueryOptions: null as null | {
    sql: string;
    params: unknown[];
    enabled: boolean;
  },
}));

vi.mock("~/db", () => ({
  useLiveQuery: ({ sql, params, enabled, mapRows }: any) => {
    mocks.liveQueryOptions = { sql, params, enabled };
    return { data: mapRows(mocks.rows) };
  },
}));

import { useSessionShareSyncStatus } from "./sync-state";

describe("useSessionShareSyncStatus", () => {
  beforeEach(() => {
    mocks.rows = [];
    mocks.liveQueryOptions = null;
  });

  it("subscribes to the exact managed share conflict row", () => {
    mocks.rows = [{ status: "conflict" }];

    expect(
      renderHook(() =>
        useSessionShareSyncStatus("owner-1", "share-1", "session-1"),
      ).result.current,
    ).toBe("conflict");
    expect(mocks.liveQueryOptions).toMatchObject({
      params: ["owner-1", "share-1", "session-1"],
      enabled: true,
    });
  });

  it("rejects an invalid durable status", () => {
    mocks.rows = [{ status: "paused" }];

    expect(() =>
      renderHook(() =>
        useSessionShareSyncStatus("owner-1", "share-1", "session-1"),
      ),
    ).toThrow("Invalid shared-note sync status");
  });
});
