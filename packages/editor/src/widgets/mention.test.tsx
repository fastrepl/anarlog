import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { type MentionItem, useMentionSearch } from "./mention";

function deferredSearch() {
  const pending = new Map<
    string,
    {
      resolve: (items: MentionItem[]) => void;
      reject: (error: Error) => void;
    }
  >();
  const handleSearch = (query: string) =>
    new Promise<MentionItem[]>((resolve, reject) => {
      pending.set(query, { resolve, reject });
    });
  return { pending, handleSearch };
}

function item(id: string): MentionItem {
  return { id, type: "human", label: id };
}

afterEach(cleanup);

describe("useMentionSearch", () => {
  it("ignores an older search resolving after a newer one", async () => {
    const { pending, handleSearch } = deferredSearch();
    const { result, rerender } = renderHook(
      ({ query }) => useMentionSearch(true, query, handleSearch),
      { initialProps: { query: "a" } },
    );

    rerender({ query: "ab" });
    await act(async () => {
      pending.get("ab")!.resolve([item("newer")]);
    });
    expect(result.current.items.map((i) => i.id)).toEqual(["newer"]);

    await act(async () => {
      pending.get("a")!.resolve([item("older")]);
    });
    expect(result.current.items.map((i) => i.id)).toEqual(["newer"]);
  });

  it("ignores an older search rejecting after a newer one resolved", async () => {
    const { pending, handleSearch } = deferredSearch();
    const { result, rerender } = renderHook(
      ({ query }) => useMentionSearch(true, query, handleSearch),
      { initialProps: { query: "a" } },
    );

    rerender({ query: "ab" });
    await act(async () => {
      pending.get("ab")!.resolve([item("newer")]);
    });

    await act(async () => {
      pending.get("a")!.reject(new Error("slow failure"));
    });
    expect(result.current.items.map((i) => i.id)).toEqual(["newer"]);
  });

  it("does not repopulate after the mention is dismissed or cleared", async () => {
    const { pending, handleSearch } = deferredSearch();
    const { result, rerender } = renderHook(
      ({ active, query }: { active: boolean; query: string | undefined }) =>
        useMentionSearch(active, query, handleSearch),
      { initialProps: { active: true, query: "a" } },
    );

    rerender({ active: false, query: undefined });
    expect(result.current.items).toEqual([]);

    await act(async () => {
      pending.get("a")!.resolve([item("stale")]);
    });
    expect(result.current.items).toEqual([]);
    expect(result.current.selectedIndex).toBe(0);
  });

  it("ignores a search resolving after unmount", async () => {
    const { pending, handleSearch } = deferredSearch();
    const { unmount } = renderHook(() =>
      useMentionSearch(true, "a", handleSearch),
    );

    unmount();
    await act(async () => {
      pending.get("a")!.resolve([item("stale")]);
    });
  });

  it("resets the selection when fresh results arrive", async () => {
    const { pending, handleSearch } = deferredSearch();
    const { result, rerender } = renderHook(
      ({ query }) => useMentionSearch(true, query, handleSearch),
      { initialProps: { query: "a" } },
    );

    await act(async () => {
      pending.get("a")!.resolve([item("one"), item("two")]);
    });
    act(() => {
      result.current.setSelectedIndex(1);
    });
    expect(result.current.selectedIndex).toBe(1);

    rerender({ query: "ab" });
    await act(async () => {
      pending.get("ab")!.resolve([item("three")]);
    });
    expect(result.current.selectedIndex).toBe(0);
    expect(result.current.items.map((i) => i.id)).toEqual(["three"]);
  });

  it("caps results at five items", async () => {
    const { pending, handleSearch } = deferredSearch();
    const { result } = renderHook(() =>
      useMentionSearch(true, "a", handleSearch),
    );

    await act(async () => {
      pending.get("a")!.resolve(["1", "2", "3", "4", "5", "6", "7"].map(item));
    });
    expect(result.current.items).toHaveLength(5);
  });
});
