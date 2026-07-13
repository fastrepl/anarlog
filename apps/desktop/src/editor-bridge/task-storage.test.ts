import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TaskRecord, TaskSource } from "@hypr/editor/tasks";

import {
  createSqliteTaskStorage,
  type SqliteTaskRow,
  type TaskStorageDependencies,
} from "./task-storage";

import { DEFAULT_USER_ID } from "~/shared/utils";

const sessionSource: TaskSource = { type: "session", id: "session-1" };
const task: TaskRecord = {
  taskId: "task-1",
  sourceId: "session-1",
  sourceType: "session",
  sourceOrder: 0,
  status: "todo",
  textPreview: "Follow up",
  body: [
    {
      type: "paragraph",
      content: [{ type: "text", text: "Follow up" }],
    },
  ],
  dueDate: "2026-07-12",
};

function sqliteTaskRow(overrides: Partial<SqliteTaskRow> = {}): SqliteTaskRow {
  return {
    id: "task-1",
    source_type: "session",
    source_id: "session-1",
    source_order: 0,
    status: "todo",
    text: "Follow up",
    body_json:
      '[{"type":"paragraph","content":[{"type":"text","text":"Follow up"}]}]',
    due_at: "2026-07-12",
    ...overrides,
  };
}

function createHarness() {
  const subscriptions: Array<{
    onData: (rows: SqliteTaskRow[]) => void;
    onError?: (error: string) => void;
  }> = [];
  const unsubscribe = vi.fn().mockResolvedValue(undefined);
  const executeTransaction = vi.fn().mockResolvedValue([1]);
  const enqueueWrite = vi.fn(
    async (_key: string, write: () => Promise<void>) => {
      await write();
    },
  );
  const subscribe = (async (
    _sql: string,
    _params: unknown[],
    options: {
      onData: (rows: SqliteTaskRow[]) => void;
      onError?: (error: string) => void;
    },
  ) => {
    subscriptions.push(options);
    return unsubscribe;
  }) as TaskStorageDependencies["subscribe"];
  const dependencies: TaskStorageDependencies = {
    subscribe,
    executeTransaction,
    enqueueWrite,
    now: () => "2026-07-10T10:00:00.000Z",
  };

  return {
    dependencies,
    enqueueWrite,
    executeTransaction,
    subscriptions,
    unsubscribe,
  };
}

describe("SQLite task storage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("publishes stable source snapshots from one SQLite subscription", async () => {
    const harness = createHarness();
    const storage = createSqliteTaskStorage("user-1", harness.dependencies);
    const listener = vi.fn();

    const unsubscribe = storage.subscribeSource(sessionSource, listener);
    expect(harness.subscriptions).toHaveLength(1);
    expect(storage.getTasksForSource(sessionSource)).toEqual([]);

    harness.subscriptions[0].onData([sqliteTaskRow()]);
    const firstSnapshot = storage.getTasksForSource(sessionSource);
    expect(firstSnapshot).toEqual([task]);
    expect(storage.getTask("task-1")).toBe(firstSnapshot[0]);
    expect(listener).toHaveBeenCalledOnce();

    harness.subscriptions[0].onData([sqliteTaskRow()]);
    expect(storage.getTasksForSource(sessionSource)).toBe(firstSnapshot);
    expect(listener).toHaveBeenCalledOnce();

    harness.subscriptions[0].onData([
      sqliteTaskRow({ text: "Updated follow up" }),
    ]);
    expect(listener).toHaveBeenCalledTimes(2);
    expect(storage.getTask("task-1")?.textPreview).toBe("Updated follow up");

    unsubscribe();
    await vi.waitFor(() => expect(harness.unsubscribe).toHaveBeenCalledOnce());
  });

  it("atomically tombstones removed rows and upserts the source snapshot", async () => {
    const harness = createHarness();
    const storage = createSqliteTaskStorage("user-1", harness.dependencies);

    storage.upsertTasksForSource(sessionSource, [task]);

    await vi.waitFor(() =>
      expect(harness.executeTransaction).toHaveBeenCalledOnce(),
    );
    expect(harness.enqueueWrite).toHaveBeenCalledWith(
      "tasks",
      expect.any(Function),
    );
    const statements = harness.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("SET deleted_at = ?");
    expect(statements[0].sql).toContain("$.account_user_id");
    expect(statements[0].sql).toContain("json_each(?)");
    expect(statements[1].sql).toContain("INSERT INTO action_items");
    expect(statements[1].sql).toContain("SELECT NULLIF(workspace_id");
    expect(statements[1].sql).toContain("cloudsync_workspace_binding");
    expect(statements[1].sql).toContain("$.account_user_id");
    expect(statements[1].sql).toContain("NULLIF(workspace_id, '')");
    expect(statements[1].sql).not.toContain("\n          ''\n");
    expect(statements[1].sql).toContain("deleted_at = NULL");
    expect(statements[1].params).toContain("task-1");
    expect(statements[1].params).toContain("Follow up");
  });

  it("resolves legacy default task ownership from the workspace binding", async () => {
    const harness = createHarness();
    const storage = createSqliteTaskStorage(
      DEFAULT_USER_ID,
      harness.dependencies,
    );

    storage.upsertTasksForSource(sessionSource, [task]);

    await vi.waitFor(() =>
      expect(harness.executeTransaction).toHaveBeenCalledOnce(),
    );
    const statements = harness.executeTransaction.mock.calls[0][0];
    expect(statements[0].sql).toContain("$.account_user_id");
    expect(statements[1].sql).toContain("$.account_user_id");
    expect(
      statements.flatMap(
        (statement: { params: unknown[] }) => statement.params,
      ),
    ).toContain(DEFAULT_USER_ID);
  });

  it("skips writes when the committed source snapshot is unchanged", () => {
    const harness = createHarness();
    const storage = createSqliteTaskStorage("user-1", harness.dependencies);
    storage.subscribeSource(sessionSource, vi.fn());
    harness.subscriptions[0].onData([sqliteTaskRow()]);

    storage.upsertTasksForSource(sessionSource, [task]);

    expect(harness.enqueueWrite).not.toHaveBeenCalled();
    expect(harness.executeTransaction).not.toHaveBeenCalled();
  });

  it("scopes removals to the source and moves tasks in one batch", async () => {
    const harness = createHarness();
    const storage = createSqliteTaskStorage("user-1", harness.dependencies);

    storage.removeTasksForSource(sessionSource, ["task-1"]);
    storage.moveTasksToSource(
      ["task-1", "task-2"],
      { type: "document", id: "document-1" },
      4,
    );

    await vi.waitFor(() =>
      expect(harness.executeTransaction).toHaveBeenCalledTimes(2),
    );
    const removal = harness.executeTransaction.mock.calls[0][0][0];
    expect(removal.sql).toContain("source_type = ?");
    expect(removal.sql).toContain("id IN (SELECT value FROM json_each(?))");
    expect(removal.params.slice(-3)).toEqual([
      "session",
      "session-1",
      '["task-1"]',
    ]);

    const moves = harness.executeTransaction.mock.calls[1][0];
    expect(moves).toHaveLength(2);
    expect(moves[0].params).toEqual([
      "",
      "document",
      "document-1",
      4,
      "2026-07-10T10:00:00.000Z",
      "user-1",
      DEFAULT_USER_ID,
      "task-1",
    ]);
    expect(moves[1].params[3]).toBe(5);
  });

  it("drops malformed rows instead of exposing invalid task records", () => {
    const harness = createHarness();
    const storage = createSqliteTaskStorage("user-1", harness.dependencies);
    const listener = vi.fn();
    storage.subscribeSource(sessionSource, listener);

    harness.subscriptions[0].onData([
      sqliteTaskRow({ id: "", status: "unknown" }),
    ]);

    expect(storage.getTasksForSource(sessionSource)).toEqual([]);
    expect(storage.getTask("task-1")).toBeNull();
    expect(listener).not.toHaveBeenCalled();
  });
});
