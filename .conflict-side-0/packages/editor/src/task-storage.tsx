import {
  createContext,
  useContext,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import {
  createTaskSourceKey,
  isSameTask,
  type TaskRecord,
  type TaskSource,
} from "./tasks";

type Listener = () => void;

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
  const value = useMemo(
    () => storage ?? createInMemoryTaskStorage(),
    [storage],
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

function subscribeNoop() {
  return () => {};
}

function getEmptyTasks() {
  return emptyTasks;
}

function getNullTaskRecord() {
  return null;
}
