import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SqliteTableShadowConfig } from "./sqlite-table-shadow";
import { SqliteTableShadow } from "./sqlite-table-shadow";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

const mocks = vi.hoisted(() => ({
  saveHandlers: new Map<string, () => Promise<void>>(),
  subscribe: vi.fn(),
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    subscribe: mocks.subscribe,
  },
}));

vi.mock("./save", () => ({
  registerSaveHandler: (id: string, handler: () => Promise<void>) => {
    mocks.saveHandlers.set(id, handler);
    return () => mocks.saveHandlers.delete(id);
  },
}));

type SqliteTagRow = {
  id: string;
  owner_user_id: string;
  name: string;
  deleted_at: string | null;
};

describe("SqliteTableShadow", () => {
  let onData: (rows: SqliteTagRow[]) => void;
  let persist: SqliteTableShadowConfig<"tags", SqliteTagRow>["persist"];
  let config: SqliteTableShadowConfig<"tags", SqliteTagRow>;

  beforeEach(() => {
    persist = vi.fn(async () => {});
    config = {
      label: "TestTagShadow",
      tableId: "tags",
      selectSql: "SELECT id, owner_user_id, name FROM tags",
      fromSqlite: (row) => ({
        user_id: row.owner_user_id,
        name: row.name,
      }),
      isDeleted: (row) => row.deleted_at !== null,
      normalize: (row) => ({
        user_id: row.user_id ?? "",
        name: row.name ?? "",
      }),
      persist,
    };
    mocks.subscribe.mockImplementation(async (_sql, _params, options) => {
      onData = options.onData;
      return async () => {};
    });
  });

  afterEach(() => {
    cleanup();
    mocks.saveHandlers.clear();
    vi.clearAllMocks();
  });

  it("hydrates a SQLite-only row into the TinyBase compatibility cache", async () => {
    const store = createTestMainStore();
    render(<SqliteTableShadow config={config} store={store} />);

    await waitFor(() => expect(mocks.subscribe).toHaveBeenCalledOnce());
    act(() => {
      onData([
        {
          id: "tag-1",
          owner_user_id: "user-1",
          name: "work",
          deleted_at: null,
        },
      ]);
    });

    expect(store.getRow("tags", "tag-1")).toEqual({
      user_id: "user-1",
      name: "work",
    });
    expect(persist).not.toHaveBeenCalled();
  });

  it("lets verified SQLite data replace a stale legacy cache row", async () => {
    const store = createTestMainStore();
    store.setRow("tags", "tag-1", { user_id: "user-1", name: "old" });
    render(<SqliteTableShadow config={config} store={store} />);

    await waitFor(() => expect(mocks.subscribe).toHaveBeenCalledOnce());
    act(() => {
      onData([
        {
          id: "tag-1",
          owner_user_id: "user-1",
          name: "current",
          deleted_at: null,
        },
      ]);
    });

    expect(store.getCell("tags", "tag-1", "name")).toBe("current");
    expect(persist).not.toHaveBeenCalled();
  });

  it("preserves and persists a row created before the first snapshot", async () => {
    const store = createTestMainStore();
    store.setRow("tags", "tag-new", { user_id: "user-1", name: "new" });
    render(<SqliteTableShadow config={config} store={store} />);

    await waitFor(() => expect(mocks.subscribe).toHaveBeenCalledOnce());
    act(() => onData([]));

    await waitFor(() => {
      expect(persist).toHaveBeenCalledWith(
        [["tag-new", { user_id: "user-1", name: "new" }]],
        [],
      );
    });
    expect(store.hasRow("tags", "tag-new")).toBe(true);
  });

  it("forwards a compatibility-cache deletion to SQLite", async () => {
    const store = createTestMainStore();
    render(<SqliteTableShadow config={config} store={store} />);

    await waitFor(() => expect(mocks.subscribe).toHaveBeenCalledOnce());
    act(() => {
      onData([
        {
          id: "tag-1",
          owner_user_id: "user-1",
          name: "work",
          deleted_at: null,
        },
      ]);
      store.delRow("tags", "tag-1");
    });

    await waitFor(() => {
      expect(persist).toHaveBeenCalledWith([], ["tag-1"]);
    });
  });

  it("does not resurrect a tombstoned SQLite row from the legacy cache", async () => {
    const store = createTestMainStore();
    store.setRow("tags", "tag-1", { user_id: "user-1", name: "stale" });
    render(<SqliteTableShadow config={config} store={store} />);

    await waitFor(() => expect(mocks.subscribe).toHaveBeenCalledOnce());
    act(() => {
      onData([
        {
          id: "tag-1",
          owner_user_id: "user-1",
          name: "stale",
          deleted_at: "2026-07-10T02:00:00Z",
        },
      ]);
    });

    expect(store.hasRow("tags", "tag-1")).toBe(false);
    expect(persist).not.toHaveBeenCalled();
  });

  it("keeps the app save barrier open until a pending SQLite write finishes", async () => {
    let releaseWrite: (() => void) | undefined;
    persist = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          releaseWrite = resolve;
        }),
    );
    config = { ...config, persist };
    const store = createTestMainStore();
    render(<SqliteTableShadow config={config} store={store} />);

    await waitFor(() => expect(mocks.subscribe).toHaveBeenCalledOnce());
    act(() => {
      onData([
        {
          id: "tag-1",
          owner_user_id: "user-1",
          name: "work",
          deleted_at: null,
        },
      ]);
      store.setCell("tags", "tag-1", "name", "updated");
    });

    const flush = mocks.saveHandlers.get("sqlite-shadow:tags");
    expect(flush).toBeDefined();
    const flushPromise = flush!();
    let settled = false;
    void flushPromise.then(() => {
      settled = true;
    });

    await waitFor(() => expect(persist).toHaveBeenCalled());
    expect(settled).toBe(false);
    releaseWrite?.();
    await flushPromise;
    expect(settled).toBe(true);
  });
});
