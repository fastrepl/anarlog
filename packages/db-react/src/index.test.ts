import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LiveQueryClient } from "@hypr/db-runtime";

import { createUseDrizzleLiveQuery, createUseLiveQuery } from "./index";

describe("@hypr/db-react", () => {
  const subscribeMock = vi.fn<LiveQueryClient["subscribe"]>();
  const client: LiveQueryClient = {
    execute: vi.fn(),
    subscribe: subscribeMock,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads initial rows and unsubscribes on unmount", async () => {
    const useLiveQuery = createUseLiveQuery(client);
    const unsubscribe = vi.fn();
    let onData: ((rows: Array<{ id: number }>) => void) | undefined;

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      onData = options.onData;
      return unsubscribe;
    });

    const { result, unmount } = renderHook(() =>
      useLiveQuery({
        sql: "SELECT id FROM test",
        params: [1],
        mapRows: (rows: Array<{ id: number }>) => rows.map((row) => row.id),
      }),
    );

    expect(result.current.isLoading).toBe(true);

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(1);
    });

    act(() => {
      onData?.([{ id: 1 }, { id: 2 }]);
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
      expect(result.current.data).toEqual([1, 2]);
      expect(result.current.error).toBeNull();
    });

    unmount();

    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("does not subscribe when disabled", () => {
    const useLiveQuery = createUseLiveQuery(client);
    const { result } = renderHook(() =>
      useLiveQuery({
        sql: "SELECT id FROM test",
        enabled: false,
      }),
    );

    expect(subscribeMock).not.toHaveBeenCalled();
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("ignores late subscription resolution after unmount", async () => {
    const useLiveQuery = createUseLiveQuery(client);
    const unsubscribe = vi.fn();
    let resolveSubscribe: ((value: () => void) => void) | undefined;

    subscribeMock.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve;
        }),
    );

    const { unmount } = renderHook(() =>
      useLiveQuery({
        sql: "SELECT id FROM test",
      }),
    );

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(1);
    });

    unmount();
    resolveSubscribe?.(unsubscribe);

    await waitFor(() => {
      expect(unsubscribe).toHaveBeenCalledTimes(1);
    });
  });

  it("uses query.toSQL in the Drizzle wrapper", async () => {
    const useDrizzleLiveQuery = createUseDrizzleLiveQuery(client);
    let onData: ((rows: Array<{ id: string }>) => void) | undefined;

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      onData = options.onData;
      return () => {};
    });

    const query = {
      toSQL: () => ({
        sql: "SELECT id FROM templates WHERE pinned = ?",
        params: [true],
      }),
    };

    const { result } = renderHook(() =>
      useDrizzleLiveQuery(query, {
        mapRows: (rows: Array<{ id: string }>) => rows.map((row) => row.id),
      }),
    );

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledWith(
        "SELECT id FROM templates WHERE pinned = ?",
        [true],
        expect.any(Object),
      );
    });

    act(() => {
      onData?.([{ id: "template-1" }]);
    });

    await waitFor(() => {
      expect(result.current.data).toEqual(["template-1"]);
      expect(result.current.isLoading).toBe(false);
    });
  });
});
