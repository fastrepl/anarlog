import { useMemo } from "react";

import type { JSONContent } from "@hypr/editor/note";
import type { TaskStorage } from "@hypr/editor/task-storage";
import {
  createTaskSourceKey,
  isSameTask,
  type TaskRecord,
  type TaskSource,
} from "@hypr/editor/tasks";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { useOwnerUserId } from "~/shared/owner-user";
import { DEFAULT_USER_ID } from "~/shared/utils";

export type SqliteTaskRow = {
  id: string;
  source_type: string;
  source_id: string;
  source_order: number;
  status: string;
  text: string;
  body_json: string;
  due_at: string;
};

export type TaskStorageDependencies = {
  subscribe: typeof liveQueryClient.subscribe;
  executeTransaction: typeof executeTransaction;
  enqueueWrite: (key: string, write: () => Promise<void>) => Promise<void>;
  now: () => string;
};

type SourceSubscription = {
  active: boolean;
  listeners: Set<() => void>;
  unsubscribePromise: Promise<() => Promise<void>>;
};

const emptyTasks: TaskRecord[] = [];

const defaultDependencies: TaskStorageDependencies = {
  subscribe: liveQueryClient.subscribe.bind(liveQueryClient),
  executeTransaction,
  enqueueWrite: enqueueDatabaseWrite,
  now: () => new Date().toISOString(),
};

export function useStoreBackedTaskStorage(): TaskStorage {
  const ownerUserId = useOwnerUserId();

  return useMemo(
    () => createSqliteTaskStorage(ownerUserId || DEFAULT_USER_ID),
    [ownerUserId],
  );
}

export function createSqliteTaskStorage(
  ownerUserId = DEFAULT_USER_ID,
  dependencies: TaskStorageDependencies = defaultDependencies,
): TaskStorage {
  const sourceSnapshots = new Map<string, TaskRecord[]>();
  const taskSnapshots = new Map<string, TaskRecord>();
  const subscriptions = new Map<string, SourceSubscription>();

  const updateSourceSnapshot = (
    source: TaskSource,
    rows: SqliteTaskRow[],
  ): boolean => {
    const sourceKey = createTaskSourceKey(source);
    const previousTasks = sourceSnapshots.get(sourceKey) ?? emptyTasks;
    const nextTasks = rows
      .map(sqliteTaskRowToRecord)
      .filter((task): task is TaskRecord => task !== null);
    if (areSameTaskSets(previousTasks, nextTasks)) {
      return false;
    }

    sourceSnapshots.set(sourceKey, nextTasks);
    const nextTaskIds = new Set(nextTasks.map((task) => task.taskId));
    previousTasks.forEach((task) => {
      const cachedTask = taskSnapshots.get(task.taskId);
      if (
        !nextTaskIds.has(task.taskId) &&
        cachedTask?.sourceType === source.type &&
        cachedTask.sourceId === source.id
      ) {
        taskSnapshots.delete(task.taskId);
      }
    });
    nextTasks.forEach((task) => {
      const previousTask = taskSnapshots.get(task.taskId);
      taskSnapshots.set(
        task.taskId,
        previousTask && isSameTask(previousTask, task) ? previousTask : task,
      );
    });
    return true;
  };

  return {
    getTasksForSource(source) {
      return sourceSnapshots.get(createTaskSourceKey(source)) ?? emptyTasks;
    },
    subscribeSource(source, listener) {
      const sourceKey = createTaskSourceKey(source);
      let subscription = subscriptions.get(sourceKey);

      if (!subscription) {
        const listeners = new Set<() => void>();
        subscription = {
          active: true,
          listeners,
          unsubscribePromise: Promise.resolve(async () => {}),
        };
        const currentSubscription = subscription;
        currentSubscription.unsubscribePromise = dependencies
          .subscribe<SqliteTaskRow>(
            `
            SELECT
              id,
              source_type,
              source_id,
              source_order,
              status,
              text,
              body_json,
              due_at
            FROM action_items
            WHERE source_type = ? AND source_id = ? AND deleted_at IS NULL
            ORDER BY source_order, id
          `,
            [source.type, source.id],
            {
              onData: (rows) => {
                if (
                  currentSubscription.active &&
                  updateSourceSnapshot(source, rows)
                ) {
                  currentSubscription.listeners.forEach((notify) => notify());
                }
              },
              onError: (error) => {
                console.error(
                  `[tasks] failed to subscribe to ${sourceKey}`,
                  error,
                );
              },
            },
          )
          .catch((error) => {
            if (currentSubscription.active) {
              console.error(
                `[tasks] failed to start subscription for ${sourceKey}`,
                error,
              );
            }
            return async () => {};
          });
        subscriptions.set(sourceKey, currentSubscription);
      }

      subscription.listeners.add(listener);
      const currentSubscription = subscription;
      return () => {
        currentSubscription.listeners.delete(listener);
        if (currentSubscription.listeners.size > 0) {
          return;
        }

        currentSubscription.active = false;
        if (subscriptions.get(sourceKey) === currentSubscription) {
          subscriptions.delete(sourceKey);
        }
        void currentSubscription.unsubscribePromise
          .then((unsubscribe) => unsubscribe())
          .catch((error) => {
            console.error(
              `[tasks] failed to unsubscribe from ${sourceKey}`,
              error,
            );
          });
      };
    },
    getTask(taskId) {
      return taskSnapshots.get(taskId) ?? null;
    },
    upsertTasksForSource(source, tasks) {
      const currentTasks =
        sourceSnapshots.get(createTaskSourceKey(source)) ?? emptyTasks;
      if (areSameTaskSets(currentTasks, tasks)) {
        return;
      }

      const now = dependencies.now();
      const retainedTaskIds = tasks.map((task) => task.taskId);
      const statements = [
        {
          sql: `
            UPDATE action_items
            SET deleted_at = ?, updated_at = ?, updated_by = ?
            WHERE source_type = ?
              AND source_id = ?
              AND deleted_at IS NULL
              AND id NOT IN (SELECT value FROM json_each(?))
          `,
          params: [
            now,
            now,
            ownerUserId,
            source.type,
            source.id,
            JSON.stringify(retainedTaskIds),
          ],
        },
        ...tasks.map((task) =>
          buildTaskUpsertStatement(task, ownerUserId, now),
        ),
      ];

      persistTaskStatements(dependencies, statements);
    },
    removeTasksForSource(source, taskIds) {
      if (taskIds.length === 0) {
        return;
      }

      const now = dependencies.now();
      persistTaskStatements(dependencies, [
        {
          sql: `
            UPDATE action_items
            SET deleted_at = ?, updated_at = ?, updated_by = ?
            WHERE source_type = ?
              AND source_id = ?
              AND deleted_at IS NULL
              AND id IN (SELECT value FROM json_each(?))
          `,
          params: [
            now,
            now,
            ownerUserId,
            source.type,
            source.id,
            JSON.stringify(taskIds),
          ],
        },
      ]);
    },
    moveTasksToSource(taskIds, nextSource, insertionOrder) {
      if (taskIds.length === 0) {
        return;
      }

      const now = dependencies.now();
      persistTaskStatements(
        dependencies,
        taskIds.map((taskId, index) => ({
          sql: `
            UPDATE action_items
            SET
              session_id = ?,
              source_type = ?,
              source_id = ?,
              source_order = ?,
              updated_at = ?,
              updated_by = ?
            WHERE id = ? AND deleted_at IS NULL
          `,
          params: [
            nextSource.type === "session" ? nextSource.id : "",
            nextSource.type,
            nextSource.id,
            insertionOrder + index,
            now,
            ownerUserId,
            taskId,
          ],
        })),
      );
    },
  };
}

function persistTaskStatements(
  dependencies: TaskStorageDependencies,
  statements: Array<{ sql: string; params: unknown[] }>,
) {
  void dependencies
    .enqueueWrite("tasks", async () => {
      await dependencies.executeTransaction(statements);
    })
    .catch((error) => {
      console.error("[tasks] failed to persist task changes", error);
    });
}

function buildTaskUpsertStatement(
  task: TaskRecord,
  ownerUserId: string,
  now: string,
) {
  return {
    sql: `
      INSERT INTO action_items (
        id,
        workspace_id,
        session_id,
        source_type,
        source_id,
        source_order,
        assignee_human_id,
        status,
        text,
        body_json,
        due_at,
        created_by,
        updated_by,
        metadata_json,
        created_at,
        updated_at,
        deleted_at
      )
      VALUES (?, '', ?, ?, ?, ?, '', ?, ?, ?, ?, ?, ?, '{}', ?, ?, NULL)
      ON CONFLICT(id) DO UPDATE SET
        session_id = excluded.session_id,
        source_type = excluded.source_type,
        source_id = excluded.source_id,
        source_order = excluded.source_order,
        status = excluded.status,
        text = excluded.text,
        body_json = excluded.body_json,
        due_at = excluded.due_at,
        updated_by = excluded.updated_by,
        updated_at = excluded.updated_at,
        deleted_at = NULL
    `,
    params: [
      task.taskId,
      task.sourceType === "session" ? task.sourceId : "",
      task.sourceType,
      task.sourceId,
      task.sourceOrder,
      task.status,
      task.textPreview,
      JSON.stringify(task.body),
      task.dueDate ?? "",
      ownerUserId,
      ownerUserId,
      now,
      now,
    ],
  };
}

function sqliteTaskRowToRecord(row: SqliteTaskRow): TaskRecord | null {
  const sourceOrder = Number(row.source_order);
  if (
    !row.id ||
    !row.source_id ||
    !row.source_type ||
    !Number.isFinite(sourceOrder) ||
    (row.status !== "todo" &&
      row.status !== "in_progress" &&
      row.status !== "done")
  ) {
    return null;
  }

  const body = parseTaskBody(row.body_json, row.text);
  return {
    taskId: row.id,
    sourceId: row.source_id,
    sourceType: row.source_type,
    sourceOrder,
    status: row.status,
    textPreview: row.text || getTextPreview(body),
    body,
    dueDate: row.due_at || undefined,
  };
}

function areSameTaskSets(left: TaskRecord[], right: TaskRecord[]) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((task, index) => isSameTask(task, right[index]!));
}

function parseTaskBody(bodyJson: unknown, legacyText: unknown): JSONContent[] {
  if (typeof bodyJson === "string" && bodyJson) {
    try {
      const parsed = JSON.parse(bodyJson);
      if (Array.isArray(parsed)) {
        return parsed as JSONContent[];
      }
    } catch {}
  }

  if (typeof legacyText === "string" && legacyText) {
    return [
      {
        type: "paragraph",
        content: [{ type: "text", text: legacyText }],
      },
    ];
  }

  return [{ type: "paragraph" }];
}

function getTextPreview(body: JSONContent[]): string {
  const firstParagraph = body.find((node) => node.type === "paragraph");
  return getNodeText(firstParagraph).trim();
}

function getNodeText(node: JSONContent | undefined): string {
  if (!node) {
    return "";
  }

  if (typeof node.text === "string") {
    return node.text;
  }

  return (node.content ?? []).map((child) => getNodeText(child)).join(" ");
}
