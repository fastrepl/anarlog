import { and, asc, eq, tasks as tasksTable } from "@hypr/db";
import { electronLiveQueryClient } from "@hypr/db-electron";

import { db } from "~/db";

type Listener = () => void;
type TaskStatus = "todo" | "in_progress" | "done";

export type JSONContent = {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: JSONContent[];
  marks?: { type: string; attrs?: Record<string, unknown> }[];
  text?: string;
};

export type TaskSource = {
  type: string;
  id: string;
};

export type TaskRecord = {
  taskId: string;
  sourceId: string;
  sourceType: string;
  sourceOrder: number;
  status: TaskStatus;
  textPreview: string;
  body: JSONContent[];
  dueDate?: string;
};

export interface TaskStorage {
  getTasksForSource(source: TaskSource): TaskRecord[];
  subscribeSource(source: TaskSource, listener: Listener): () => void;
  getTask(taskId: string): TaskRecord | null;
  upsertTasksForSource(source: TaskSource, tasks: TaskRecord[]): void;
  removeTasksForSource(source: TaskSource, taskIds: string[]): void;
  moveTasksToSource(
    taskIds: string[],
    nextSource: TaskSource,
    insertionOrder: number,
  ): void;
}

export function createHyprTaskStorage(): TaskStorage {
  return new HyprTaskStorage();
}

// Imperative (non-React) mirror of the tasks table, cached per
// `(source_type, source_id)` pair. Reads come through `@hypr/db-electron`'s
// subscribe primitive; writes go through drizzle. The queue serializes
// writes so an `upsertTasksForSource` batch lands atomically from the
// renderer's perspective without requiring multi-statement transactions in
// the Electron preload.
class HyprTaskStorage implements TaskStorage {
  private readonly snapshots = new Map<string, TaskRecord[]>();
  private readonly tasksById = new Map<string, TaskRecord>();
  private readonly listeners = new Map<string, Set<Listener>>();
  private readonly unsubscribers = new Map<string, () => Promise<void>>();
  private writeQueue: Promise<void> = Promise.resolve();

  getTasksForSource(source: TaskSource): TaskRecord[] {
    return this.snapshots.get(createTaskSourceKey(source)) ?? [];
  }

  subscribeSource(source: TaskSource, listener: Listener): () => void {
    const sourceKey = createTaskSourceKey(source);
    const listeners = this.listeners.get(sourceKey) ?? new Set<Listener>();
    listeners.add(listener);
    this.listeners.set(sourceKey, listeners);

    if (!this.unsubscribers.has(sourceKey)) {
      void this.startLiveQuery(source, sourceKey);
    }

    return () => {
      const current = this.listeners.get(sourceKey);
      if (!current) {
        return;
      }

      current.delete(listener);
      if (current.size > 0) {
        return;
      }

      this.listeners.delete(sourceKey);
      const stop = this.unsubscribers.get(sourceKey);
      this.unsubscribers.delete(sourceKey);
      void stop?.();
    };
  }

  getTask(taskId: string): TaskRecord | null {
    return this.tasksById.get(taskId) ?? null;
  }

  upsertTasksForSource(source: TaskSource, tasks: TaskRecord[]): void {
    const sourceKey = createTaskSourceKey(source);
    const currentTasks = this.getTasksForSource(source);
    const currentById = new Map(
      currentTasks.map((task) => [task.taskId, task] as const),
    );
    const nextTasks = sortTasks(tasks);
    const nextIds = new Set(nextTasks.map((task) => task.taskId));

    this.setSourceSnapshot(sourceKey, nextTasks);
    this.enqueue(async () => {
      for (const task of currentTasks) {
        if (!nextIds.has(task.taskId)) {
          await db.delete(tasksTable).where(eq(tasksTable.id, task.taskId));
        }
      }

      const nowIso = new Date().toISOString();
      for (const task of nextTasks) {
        const record = taskToRow(task, nowIso);
        if (currentById.has(task.taskId)) {
          await db
            .update(tasksTable)
            .set(record)
            .where(eq(tasksTable.id, task.taskId));
        } else {
          await db.insert(tasksTable).values({ ...record, createdAt: nowIso });
        }
      }
    });
  }

  removeTasksForSource(source: TaskSource, taskIds: string[]): void {
    const sourceKey = createTaskSourceKey(source);
    const taskIdSet = new Set(taskIds);
    const nextTasks = this.getTasksForSource(source).filter(
      (task) => !taskIdSet.has(task.taskId),
    );

    this.setSourceSnapshot(sourceKey, nextTasks);
    this.enqueue(async () => {
      for (const taskId of taskIds) {
        await db.delete(tasksTable).where(eq(tasksTable.id, taskId));
      }
    });
  }

  moveTasksToSource(
    taskIds: string[],
    nextSource: TaskSource,
    insertionOrder: number,
  ): void {
    const affectedSourceKeys = new Set<string>([
      createTaskSourceKey(nextSource),
    ]);
    const movedTasks: TaskRecord[] = [];

    taskIds.forEach((taskId, index) => {
      const currentTask = this.tasksById.get(taskId);
      if (!currentTask) {
        return;
      }

      affectedSourceKeys.add(
        createTaskSourceKey({
          type: currentTask.sourceType,
          id: currentTask.sourceId,
        }),
      );
      movedTasks.push({
        ...currentTask,
        sourceType: nextSource.type,
        sourceId: nextSource.id,
        sourceOrder: insertionOrder + index,
      });
    });

    if (movedTasks.length === 0) {
      return;
    }

    for (const sourceKey of affectedSourceKeys) {
      const tasks = (this.snapshots.get(sourceKey) ?? []).filter(
        (task) => !taskIds.includes(task.taskId),
      );
      if (sourceKey === createTaskSourceKey(nextSource)) {
        tasks.push(...movedTasks);
      }
      this.setSourceSnapshot(sourceKey, tasks);
    }

    this.enqueue(async () => {
      const nowIso = new Date().toISOString();
      for (const task of movedTasks) {
        await db
          .update(tasksTable)
          .set({
            sourceType: task.sourceType,
            sourceId: task.sourceId,
            sourceOrder: task.sourceOrder,
            updatedAt: nowIso,
          })
          .where(eq(tasksTable.id, task.taskId));
      }
    });
  }

  private async startLiveQuery(
    source: TaskSource,
    sourceKey: string,
  ): Promise<void> {
    const query = db
      .select()
      .from(tasksTable)
      .where(
        and(
          eq(tasksTable.sourceType, source.type),
          eq(tasksTable.sourceId, source.id),
        ),
      )
      .orderBy(asc(tasksTable.sourceOrder), asc(tasksTable.id));
    const { sql, params } = query.toSQL();

    const unsubscribe = await electronLiveQueryClient.subscribe<
      typeof tasksTable.$inferSelect
    >(sql, params, {
      onData: (rows) => {
        this.setSourceSnapshot(sourceKey, rows.map(dbRowToRecord));
      },
      onError: (error) => {
        console.error("[desktop2] tasksForSource live query failed", error);
      },
    });

    if (this.listeners.has(sourceKey)) {
      this.unsubscribers.set(sourceKey, unsubscribe);
      return;
    }

    void unsubscribe();
  }

  private setSourceSnapshot(sourceKey: string, tasks: TaskRecord[]) {
    this.snapshots.set(sourceKey, sortTasks(tasks));
    this.rebuildTaskIndex();
    this.listeners.get(sourceKey)?.forEach((listener) => listener());
  }

  private rebuildTaskIndex() {
    this.tasksById.clear();
    for (const tasks of this.snapshots.values()) {
      tasks.forEach((task) => {
        this.tasksById.set(task.taskId, task);
      });
    }
  }

  private enqueue(operation: () => Promise<void>) {
    this.writeQueue = this.writeQueue.then(operation).catch((error) => {
      console.error("[desktop2] task write failed", error);
    });
  }
}

function dbRowToRecord(row: typeof tasksTable.$inferSelect): TaskRecord {
  return {
    taskId: row.id,
    sourceType: row.sourceType,
    sourceId: row.sourceId,
    sourceOrder: row.sourceOrder,
    status: row.status as TaskStatus,
    textPreview: row.textPreview,
    body: parseBodyJson(row.bodyJson),
    dueDate: row.dueDate ?? undefined,
  };
}

function taskToRow(task: TaskRecord, nowIso: string) {
  return {
    id: task.taskId,
    sourceType: task.sourceType,
    sourceId: task.sourceId,
    sourceOrder: task.sourceOrder,
    status: task.status,
    textPreview: task.textPreview,
    bodyJson: JSON.stringify(task.body),
    dueDate: task.dueDate ?? null,
    updatedAt: nowIso,
  };
}

function parseBodyJson(bodyJson: string): JSONContent[] {
  try {
    const parsed = JSON.parse(bodyJson);
    if (Array.isArray(parsed)) {
      return parsed as JSONContent[];
    }
  } catch {
    // Fall through to the default placeholder paragraph.
  }

  return [{ type: "paragraph" }];
}

function sortTasks(tasks: TaskRecord[]) {
  return [...tasks].sort((left, right) => left.sourceOrder - right.sourceOrder);
}

function createTaskSourceKey(source: TaskSource): string {
  return `${source.type}:${source.id}`;
}
