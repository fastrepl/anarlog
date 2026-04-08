import {
  createContext,
  useContext,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import type { JSONContent } from "./session";
import { createTaskSourceKey, type TaskRecord, type TaskSource } from "./tasks";

import { DEFAULT_USER_ID } from "~/shared/utils";
import * as main from "~/store/tinybase/store/main";

type Listener = () => void;
type TaskStore = NonNullable<ReturnType<typeof main.UI.useStore>>;
type TaskIndexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;

export interface TaskStorage {
  getTasksForSource: (source: TaskSource) => TaskRecord[];
  subscribeSource: (source: TaskSource, listener: Listener) => () => void;
  getTask: (taskId: string) => TaskRecord | null;
  upsertTasksForSource: (source: TaskSource, tasks: TaskRecord[]) => void;
  removeTasksForSource: (source: TaskSource, taskIds: string[]) => void;
  moveTasksToSource: (
    taskIds: string[],
    nextSource: TaskSource,
    insertionOrder: number,
  ) => void;
}

const emptyTasks: TaskRecord[] = [];

const TaskStorageContext = createContext<TaskStorage | null>(null);

export function createInMemoryTaskStorage(): TaskStorage {
  const tasksById = new Map<string, TaskRecord>();
  const listenersBySource = new Map<string, Set<Listener>>();
  const sourceSnapshots = new Map<string, TaskRecord[]>();

  const emitSources = (sourceKeys: Iterable<string>) => {
    for (const sourceKey of new Set(sourceKeys)) {
      listenersBySource.get(sourceKey)?.forEach((listener) => listener());
    }
  };

  const refreshSourceSnapshot = (source: TaskSource) => {
    const sourceKey = createTaskSourceKey(source);
    sourceSnapshots.set(
      sourceKey,
      [...tasksById.values()]
        .filter(
          (task) =>
            task.sourceId === source.id && task.sourceType === source.type,
        )
        .sort((left, right) => left.sourceOrder - right.sourceOrder),
    );
  };

  const getTasksForSource = (source: TaskSource): TaskRecord[] => {
    const sourceKey = createTaskSourceKey(source);
    const snapshot = sourceSnapshots.get(sourceKey);

    if (snapshot) {
      return snapshot;
    }

    refreshSourceSnapshot(source);
    return sourceSnapshots.get(sourceKey) ?? emptyTasks;
  };

  return {
    getTasksForSource,
    subscribeSource(source, listener) {
      const sourceKey = createTaskSourceKey(source);
      const listeners = listenersBySource.get(sourceKey) ?? new Set<Listener>();
      listeners.add(listener);
      listenersBySource.set(sourceKey, listeners);

      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          listenersBySource.delete(sourceKey);
        }
      };
    },
    getTask(taskId) {
      return tasksById.get(taskId) ?? null;
    },
    upsertTasksForSource(source, tasks) {
      const sourceKey = createTaskSourceKey(source);
      const affectedSources = new Set<string>([sourceKey]);
      const nextTaskIds = new Set(tasks.map((task) => task.taskId));
      let changed = false;

      for (const [taskId, task] of tasksById.entries()) {
        if (
          task.sourceId === source.id &&
          task.sourceType === source.type &&
          !nextTaskIds.has(taskId)
        ) {
          tasksById.delete(taskId);
          changed = true;
        }
      }

      tasks.forEach((task) => {
        const previousTask = tasksById.get(task.taskId);
        if (previousTask) {
          affectedSources.add(
            createTaskSourceKey({
              type: previousTask.sourceType,
              id: previousTask.sourceId,
            }),
          );
        }
        if (!previousTask || !isSameTask(previousTask, task)) {
          tasksById.set(task.taskId, task);
          changed = true;
        }
      });

      if (!changed) {
        return;
      }

      affectedSources.forEach((nextSourceKey) => {
        const [type, ...idParts] = nextSourceKey.split(":");
        refreshSourceSnapshot({ type, id: idParts.join(":") });
      });
      emitSources(affectedSources);
    },
    removeTasksForSource(source, taskIds) {
      const sourceKey = createTaskSourceKey(source);
      const taskIdSet = new Set(taskIds);
      let changed = false;

      for (const [taskId, task] of tasksById.entries()) {
        if (
          taskIdSet.has(taskId) &&
          task.sourceId === source.id &&
          task.sourceType === source.type
        ) {
          tasksById.delete(taskId);
          changed = true;
        }
      }

      if (changed) {
        refreshSourceSnapshot(source);
        emitSources([sourceKey]);
      }
    },
    moveTasksToSource(taskIds, nextSource, insertionOrder) {
      const affectedSources = new Set<string>([
        createTaskSourceKey(nextSource),
      ]);
      let changed = false;

      taskIds.forEach((taskId, index) => {
        const previousTask = tasksById.get(taskId);
        if (!previousTask) {
          return;
        }

        affectedSources.add(
          createTaskSourceKey({
            type: previousTask.sourceType,
            id: previousTask.sourceId,
          }),
        );
        const nextTask = {
          ...previousTask,
          sourceId: nextSource.id,
          sourceType: nextSource.type,
          sourceOrder: insertionOrder + index,
        };
        if (!isSameTask(previousTask, nextTask)) {
          tasksById.set(taskId, nextTask);
          changed = true;
        }
      });

      if (!changed) {
        return;
      }

      affectedSources.forEach((sourceKey) => {
        const [type, ...idParts] = sourceKey.split(":");
        refreshSourceSnapshot({ type, id: idParts.join(":") });
      });
      emitSources(affectedSources);
    },
  };
}

export function TaskStorageProvider({
  storage,
  children,
}: {
  storage?: TaskStorage;
  children: ReactNode;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const defaultStorage = useMemo(
    () =>
      store && indexes
        ? createStoreBackedTaskStorage(store, indexes)
        : createInMemoryTaskStorage(),
    [indexes, store],
  );

  const value = useMemo(
    () => storage ?? defaultStorage,
    [defaultStorage, storage],
  );

  return (
    <TaskStorageContext.Provider value={value}>
      {children}
    </TaskStorageContext.Provider>
  );
}

export function useTaskStorageOptional(): TaskStorage | null {
  return useContext(TaskStorageContext);
}

export function useTaskStorage(): TaskStorage {
  const storage = useTaskStorageOptional();
  if (!storage) {
    throw new Error("useTaskStorage must be used within a TaskStorageProvider");
  }

  return storage;
}

export function useTaskRecords(
  source: TaskSource | null | undefined,
): TaskRecord[] {
  const storage = useTaskStorageOptional();

  return useSyncExternalStore(
    source && storage
      ? (listener) => storage.subscribeSource(source, listener)
      : subscribeNoop,
    source && storage ? () => storage.getTasksForSource(source) : getEmptyTasks,
    getEmptyTasks,
  );
}

export function useTaskRecord(
  source: TaskSource | null | undefined,
  taskId: string | null | undefined,
): TaskRecord | null {
  const storage = useTaskStorageOptional();

  return useSyncExternalStore(
    source && storage
      ? (listener) => storage.subscribeSource(source, listener)
      : subscribeNoop,
    source && taskId && storage
      ? () => storage.getTask(taskId)
      : getNullTaskRecord,
    getNullTaskRecord,
  );
}

export function setTaskRecord(store: TaskStore, task: TaskRecord): void {
  store.setRow("tasks", task.taskId, {
    user_id:
      (store.getValue("user_id") as string | undefined) ?? DEFAULT_USER_ID,
    task_id: task.taskId,
    source_id: task.sourceId,
    source_type: task.sourceType,
    source_order: task.sourceOrder,
    status: task.status,
    text_preview: task.textPreview,
    body_json: JSON.stringify(task.body),
    due_date: task.dueDate ?? "",
  });
}

function createStoreBackedTaskStorage(
  store: TaskStore,
  indexes: TaskIndexes,
): TaskStorage {
  const sourceSnapshots = new Map<string, TaskRecord[]>();
  const taskSnapshots = new Map<string, TaskRecord | null>();

  const refreshSourceSnapshot = (source: TaskSource) => {
    const sourceKey = createTaskSourceKey(source);
    const previousTasks = sourceSnapshots.get(sourceKey) ?? emptyTasks;
    const nextTasks = getTaskRecordsForSource(store, indexes, source);
    const tasks = areSameTaskSets(previousTasks, nextTasks)
      ? previousTasks
      : nextTasks;
    sourceSnapshots.set(sourceKey, tasks);

    const previousTaskIds = new Set(previousTasks.map((task) => task.taskId));
    tasks.forEach((task) => {
      const previousTask = taskSnapshots.get(task.taskId);
      taskSnapshots.set(
        task.taskId,
        previousTask && isSameTask(previousTask, task) ? previousTask : task,
      );
      previousTaskIds.delete(task.taskId);
    });
    previousTaskIds.forEach((taskId) => {
      taskSnapshots.delete(taskId);
    });
  };

  const invalidateTaskSnapshot = (taskId: string) => {
    taskSnapshots.delete(taskId);
  };

  return {
    getTasksForSource(source) {
      const sourceKey = createTaskSourceKey(source);
      const snapshot = sourceSnapshots.get(sourceKey);

      if (snapshot) {
        return snapshot;
      }

      refreshSourceSnapshot(source);
      return sourceSnapshots.get(sourceKey) ?? emptyTasks;
    },
    subscribeSource(source, listener) {
      const sourceKey = createTaskSourceKey(source);
      const rowListenerIds = new Map<string, string | number>();
      let notify = () => {};

      const refreshRowListeners = () => {
        const nextRowIds = new Set(getTaskRowIdsForSource(indexes, source));

        for (const [rowId, listenerId] of rowListenerIds.entries()) {
          if (!nextRowIds.has(rowId)) {
            store.delListener(String(listenerId));
            rowListenerIds.delete(rowId);
          }
        }

        nextRowIds.forEach((rowId) => {
          if (rowListenerIds.has(rowId)) {
            return;
          }

          rowListenerIds.set(
            rowId,
            store.addRowListener("tasks", rowId, () => {
              notify();
            }),
          );
        });
      };

      notify = () => {
        refreshRowListeners();
        refreshSourceSnapshot(source);
        listener();
      };

      refreshRowListeners();
      refreshSourceSnapshot(source);
      const sliceListenerId = indexes.addSliceRowIdsListener(
        main.INDEXES.tasksBySource,
        sourceKey,
        notify,
      );

      return () => {
        store.delListener(String(sliceListenerId));
        rowListenerIds.forEach((listenerId) => {
          store.delListener(String(listenerId));
        });
      };
    },
    getTask(taskId) {
      if (taskSnapshots.has(taskId)) {
        return taskSnapshots.get(taskId) ?? null;
      }

      const row = store.getRow("tasks", taskId);
      const task = row ? taskRowToRecord(taskId, row) : null;
      taskSnapshots.set(taskId, task);
      return task;
    },
    upsertTasksForSource(source, tasks) {
      const sourceRowIds = getTaskRowIdsForSource(indexes, source);
      const nextTaskIds = new Set(tasks.map((task) => task.taskId));
      const currentTasks = getTaskRecordsForSource(store, indexes, source);
      if (areSameTaskSets(currentTasks, tasks)) {
        return;
      }

      store.transaction(() => {
        sourceRowIds.forEach((rowId) => {
          if (!nextTaskIds.has(rowId)) {
            store.delRow("tasks", rowId);
          }
        });

        tasks.forEach((task) => {
          setTaskRecord(store, task);
          taskSnapshots.set(task.taskId, task);
        });
      });
      refreshSourceSnapshot(source);
    },
    removeTasksForSource(source, taskIds) {
      const sourceTaskIds = new Set(getTaskRowIdsForSource(indexes, source));

      store.transaction(() => {
        taskIds.forEach((taskId) => {
          if (sourceTaskIds.has(taskId)) {
            store.delRow("tasks", taskId);
            invalidateTaskSnapshot(taskId);
          }
        });
      });
      refreshSourceSnapshot(source);
    },
    moveTasksToSource(taskIds, nextSource, insertionOrder) {
      const affectedSources = new Set<string>([
        createTaskSourceKey(nextSource),
      ]);
      const updates = taskIds
        .map((taskId, index) => {
          const currentTask =
            taskSnapshots.get(taskId) ?? getTaskRecord(store, taskId);
          if (!currentTask) {
            return null;
          }

          const nextTask = {
            ...currentTask,
            sourceId: nextSource.id,
            sourceType: nextSource.type,
            sourceOrder: insertionOrder + index,
          };

          return isSameTask(currentTask, nextTask)
            ? null
            : { taskId, currentTask, nextTask };
        })
        .filter(
          (
            update,
          ): update is {
            taskId: string;
            currentTask: TaskRecord;
            nextTask: TaskRecord;
          } => update !== null,
        );

      if (updates.length === 0) {
        return;
      }

      store.transaction(() => {
        updates.forEach(({ taskId, currentTask, nextTask }) => {
          if (currentTask) {
            affectedSources.add(
              createTaskSourceKey({
                type: currentTask.sourceType,
                id: currentTask.sourceId,
              }),
            );
          }
          store.setPartialRow("tasks", taskId, {
            source_id: nextTask.sourceId,
            source_type: nextTask.sourceType,
            source_order: nextTask.sourceOrder,
          });
          invalidateTaskSnapshot(taskId);
        });
      });

      affectedSources.forEach((sourceKey) => {
        const [type, ...idParts] = sourceKey.split(":");
        refreshSourceSnapshot({ type, id: idParts.join(":") });
      });
    },
  };
}

function getTaskRecordsForSource(
  store: TaskStore,
  indexes: TaskIndexes,
  source: TaskSource,
): TaskRecord[] {
  return getTaskRowIdsForSource(indexes, source)
    .map((rowId) => {
      const row = store.getRow("tasks", rowId);
      return row ? taskRowToRecord(rowId, row) : null;
    })
    .filter((task): task is TaskRecord => task !== null);
}

function getTaskRecord(store: TaskStore, taskId: string) {
  const row = store.getRow("tasks", taskId);
  return row ? taskRowToRecord(taskId, row) : null;
}

function getTaskRowIdsForSource(indexes: TaskIndexes, source: TaskSource) {
  return indexes.getSliceRowIds(
    main.INDEXES.tasksBySource,
    createTaskSourceKey(source),
  );
}

function areSameTaskSets(left: TaskRecord[], right: TaskRecord[]) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((task, index) => isSameTask(task, right[index]!));
}

function taskRowToRecord(
  rowId: string,
  row: Record<string, unknown>,
): TaskRecord | null {
  const taskId =
    typeof row.task_id === "string" && row.task_id ? row.task_id : rowId;
  const sourceId = row.source_id;
  const sourceType = row.source_type;
  const sourceOrder =
    typeof row.source_order === "number"
      ? row.source_order
      : typeof row.order === "number"
        ? row.order
        : null;
  const status = row.status;

  if (
    typeof taskId !== "string" ||
    typeof sourceId !== "string" ||
    typeof sourceType !== "string" ||
    typeof sourceOrder !== "number" ||
    (status !== "todo" && status !== "in_progress" && status !== "done")
  ) {
    return null;
  }

  const body = parseTaskBody(row.body_json, row.text);
  const textPreview =
    typeof row.text_preview === "string" && row.text_preview
      ? row.text_preview
      : getTextPreview(body);

  return {
    taskId,
    sourceId,
    sourceType,
    sourceOrder,
    status,
    textPreview,
    body,
    dueDate:
      typeof row.due_date === "string" && row.due_date
        ? row.due_date
        : undefined,
  };
}

function parseTaskBody(bodyJson: unknown, legacyText: unknown): JSONContent[] {
  if (typeof bodyJson === "string" && bodyJson) {
    try {
      const parsed = JSON.parse(bodyJson);
      if (Array.isArray(parsed)) {
        return parsed as JSONContent[];
      }
    } catch {
      // Ignore malformed legacy data.
    }
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

function isSameTask(left: TaskRecord, right: TaskRecord) {
  return (
    left.taskId === right.taskId &&
    left.sourceId === right.sourceId &&
    left.sourceType === right.sourceType &&
    left.sourceOrder === right.sourceOrder &&
    left.status === right.status &&
    left.textPreview === right.textPreview &&
    left.dueDate === right.dueDate &&
    JSON.stringify(left.body) === JSON.stringify(right.body)
  );
}

function subscribeNoop() {
  return () => {};
}

function getEmptyTasks() {
  return emptyTasks;
}

function getNullTaskRecord() {
  return null;
}
