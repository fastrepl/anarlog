import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useLiveQuery } from "./use-live-query";

const { subscribeMock } = vi.hoisted(() => ({
  subscribeMock: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  subscribe: subscribeMock,
}));

describe("useLiveQuery", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads initial rows and unsubscribes on unmount", async () => {
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

  it("resubscribes and unsubscribes the previous query when params change", async () => {
    const unsubscribes = [vi.fn(), vi.fn()];

    subscribeMock
      .mockImplementationOnce(async () => unsubscribes[0])
      .mockImplementationOnce(async () => unsubscribes[1]);

    const { rerender } = renderHook(
      ({ value }) =>
        useLiveQuery({
          sql: "SELECT id FROM test WHERE id = ?",
          params: [value],
          mapRows: (rows: Array<{ id: number }>) => rows,
        }),
      {
        initialProps: { value: 1 },
      },
    );

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(1);
    });

    rerender({ value: 2 });

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(2);
    });

    expect(unsubscribes[0]).toHaveBeenCalledTimes(1);
  });

  it("does not subscribe when disabled", () => {
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
});
