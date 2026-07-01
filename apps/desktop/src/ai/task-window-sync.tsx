import { emit, emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { getEnhancerService } from "~/services/enhancer";
import type { AITaskStore } from "~/store/zustand/ai-task";
import type { RemoteTaskState, TaskState } from "~/store/zustand/ai-task/tasks";

const TASK_SYNC_EVENT = "hypr:ai-task-sync";
const TASK_SYNC_REQUEST_EVENT = "hypr:ai-task-sync-request";
const TASK_CANCEL_EVENT = "hypr:ai-task-cancel";
const TASK_ENHANCE_EVENT = "hypr:ai-task-enhance";

type TaskSyncPayload = {
  sourceLabel: string;
  tasks: Record<string, RemoteTaskState>;
};

type TaskSyncRequestPayload = {
  sourceLabel: string;
};

type TaskCancelPayload = {
  taskId: string;
};

type TaskEnhancePayload = {
  sessionId: string;
  opts?: {
    isAuto?: boolean;
    templateId?: string | null;
    targetNoteId?: string;
    templateTitle?: string;
  };
};

export function isMainAITaskHostWindow() {
  return getCurrentWebviewWindowLabel() === "main";
}

export async function requestMainAITaskCancel(taskId: string) {
  await emitTo("main", TASK_CANCEL_EVENT, {
    taskId,
  } satisfies TaskCancelPayload);
}

export async function requestMainEnhance(
  sessionId: string,
  opts?: TaskEnhancePayload["opts"],
) {
  await emitTo("main", TASK_ENHANCE_EVENT, {
    sessionId,
    opts,
  } satisfies TaskEnhancePayload);
}

export function AITaskWindowSyncBridge({ store }: { store: AITaskStore }) {
  const isMain = isMainAITaskHostWindow();

  if (isMain) {
    return <MainAITaskWindowSyncBridge store={store} />;
  }

  return <RemoteAITaskWindowSyncBridge store={store} />;
}

function MainAITaskWindowSyncBridge({ store }: { store: AITaskStore }) {
  useEffect(() => {
    const sourceLabel = getCurrentWebviewWindowLabel();
    let active = true;
    let syncRequestUnlisten: UnlistenFn | null = null;
    let cancelUnlisten: UnlistenFn | null = null;
    let enhanceUnlisten: UnlistenFn | null = null;

    const emitSnapshot = () => {
      void emit(TASK_SYNC_EVENT, {
        sourceLabel,
        tasks: serializeEnhanceTasks(store.getState().tasks),
      } satisfies TaskSyncPayload);
    };

    const unsubscribe = store.subscribe(emitSnapshot);
    emitSnapshot();

    void listen<TaskSyncRequestPayload>(TASK_SYNC_REQUEST_EVENT, (event) => {
      if (!active || !isTaskSyncRequestPayload(event.payload)) {
        return;
      }

      void emitTo(event.payload.sourceLabel, TASK_SYNC_EVENT, {
        sourceLabel,
        tasks: serializeEnhanceTasks(store.getState().tasks),
      } satisfies TaskSyncPayload);
    }).then((unlisten) => {
      if (active) {
        syncRequestUnlisten = unlisten;
      } else {
        unlisten();
      }
    });

    void listen<TaskCancelPayload>(TASK_CANCEL_EVENT, (event) => {
      if (!active || !isTaskCancelPayload(event.payload)) {
        return;
      }

      store.getState().cancel(event.payload.taskId);
    }).then((unlisten) => {
      if (active) {
        cancelUnlisten = unlisten;
      } else {
        unlisten();
      }
    });

    void listen<TaskEnhancePayload>(TASK_ENHANCE_EVENT, (event) => {
      if (!active || !isTaskEnhancePayload(event.payload)) {
        return;
      }

      getEnhancerService()?.enhance(
        event.payload.sessionId,
        event.payload.opts,
      );
    }).then((unlisten) => {
      if (active) {
        enhanceUnlisten = unlisten;
      } else {
        unlisten();
      }
    });

    return () => {
      active = false;
      unsubscribe();
      syncRequestUnlisten?.();
      cancelUnlisten?.();
      enhanceUnlisten?.();
    };
  }, [store]);

  return null;
}

function RemoteAITaskWindowSyncBridge({ store }: { store: AITaskStore }) {
  useEffect(() => {
    const sourceLabel = getCurrentWebviewWindowLabel();
    let active = true;
    let syncUnlisten: UnlistenFn | null = null;

    void listen<TaskSyncPayload>(TASK_SYNC_EVENT, (event) => {
      if (
        !active ||
        !isTaskSyncPayload(event.payload) ||
        event.payload.sourceLabel === sourceLabel
      ) {
        return;
      }

      store.getState().syncRemoteTasks(event.payload.tasks);
    }).then((unlisten) => {
      if (active) {
        syncUnlisten = unlisten;
        void emitTo("main", TASK_SYNC_REQUEST_EVENT, {
          sourceLabel,
        } satisfies TaskSyncRequestPayload);
      } else {
        unlisten();
      }
    });

    return () => {
      active = false;
      syncUnlisten?.();
    };
  }, [store]);

  return null;
}

function serializeEnhanceTasks(tasks: Record<string, TaskState>) {
  return Object.fromEntries(
    Object.entries(tasks)
      .filter(([, task]) => task.taskType === "enhance")
      .map(([taskId, task]) => [
        taskId,
        {
          taskType: task.taskType,
          status: task.status,
          streamedText: task.streamedText,
          error: task.error
            ? { name: task.error.name, message: task.error.message }
            : undefined,
          currentStep: task.currentStep,
        } satisfies RemoteTaskState,
      ]),
  );
}

function isTaskSyncPayload(payload: unknown): payload is TaskSyncPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<TaskSyncPayload>;
  return (
    typeof candidate.sourceLabel === "string" &&
    Boolean(candidate.tasks) &&
    typeof candidate.tasks === "object"
  );
}

function isTaskSyncRequestPayload(
  payload: unknown,
): payload is TaskSyncRequestPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  return (
    typeof (payload as Partial<TaskSyncRequestPayload>).sourceLabel === "string"
  );
}

function isTaskCancelPayload(payload: unknown): payload is TaskCancelPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  return typeof (payload as Partial<TaskCancelPayload>).taskId === "string";
}

function isTaskEnhancePayload(payload: unknown): payload is TaskEnhancePayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<TaskEnhancePayload>;
  return typeof candidate.sessionId === "string";
}
