import { emit, emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { type EnhancerService, getEnhancerService } from "~/services/enhancer";
import { id } from "~/shared/utils";
import type { AITaskStore } from "~/store/zustand/ai-task";
import type { RemoteTaskState, TaskState } from "~/store/zustand/ai-task/tasks";

const TASK_SYNC_EVENT = "hypr:ai-task-sync";
const TASK_SYNC_REQUEST_EVENT = "hypr:ai-task-sync-request";
const TASK_CANCEL_EVENT = "hypr:ai-task-cancel";
const TASK_ENHANCE_EVENT = "hypr:ai-task-enhance";
const TASK_AUTO_ENHANCE_REQUEST_EVENT = "hypr:ai-task-auto-enhance-request";
const TASK_AUTO_ENHANCE_RESULT_EVENT = "hypr:ai-task-auto-enhance-result";
export const MAIN_AUTO_ENHANCE_TIMEOUT_MS = 30_000;

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
  auto?: "regenerate" | "if_empty";
  opts?: {
    isAuto?: boolean;
    templateId?: string | null;
    targetNoteId?: string;
    templateTitle?: string;
  };
};

type TaskAutoEnhanceRequestPayload = {
  requestId: string;
  sourceLabel: string;
  sessionId: string;
  mode: NonNullable<TaskEnhancePayload["auto"]>;
};

type TaskAutoEnhanceResultPayload = {
  requestId: string;
  completed: boolean;
  error: string | null;
};

export async function handleMainEnhanceRequest(
  service: Pick<EnhancerService, "enhance" | "requestAutoEnhance">,
  { sessionId, auto, opts }: TaskEnhancePayload,
) {
  if (auto) {
    await service.requestAutoEnhance(sessionId, auto);
    return;
  }

  await service.enhance(sessionId, opts);
}

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

export async function requestMainAutoEnhance(
  sessionId: string,
  mode: NonNullable<TaskEnhancePayload["auto"]>,
): Promise<void> {
  const requestId = id();
  const sourceLabel = getCurrentWebviewWindowLabel();

  return new Promise<void>((resolve, reject) => {
    let settled = false;
    let unlisten: UnlistenFn | null = null;
    let timeout: ReturnType<typeof setTimeout> | null = null;

    const finish = (error?: Error) => {
      if (settled) return;
      settled = true;
      if (timeout !== null) clearTimeout(timeout);
      unlisten?.();
      if (error) {
        reject(error);
      } else {
        resolve();
      }
    };

    timeout = setTimeout(() => {
      finish(
        new Error(
          "Main window did not acknowledge the auto-summary request in time",
        ),
      );
    }, MAIN_AUTO_ENHANCE_TIMEOUT_MS);

    void listen<TaskAutoEnhanceResultPayload>(
      TASK_AUTO_ENHANCE_RESULT_EVENT,
      (event) => {
        if (
          !isTaskAutoEnhanceResultPayload(event.payload) ||
          event.payload.requestId !== requestId
        ) {
          return;
        }

        if (event.payload.error) {
          finish(new Error(event.payload.error));
        } else if (!event.payload.completed) {
          finish(new Error("Main window did not schedule the auto-summary"));
        } else {
          finish();
        }
      },
    ).then(
      (stopListening) => {
        if (settled) {
          stopListening();
          return;
        }

        unlisten = stopListening;
        void emitTo("main", TASK_AUTO_ENHANCE_REQUEST_EVENT, {
          requestId,
          sourceLabel,
          sessionId,
          mode,
        } satisfies TaskAutoEnhanceRequestPayload).catch((error) => {
          finish(toError(error));
        });
      },
      (error) => {
        finish(toError(error));
      },
    );
  });
}

export async function handleMainAutoEnhanceRequest(
  service: Pick<EnhancerService, "requestAutoEnhance"> | null,
  request: TaskAutoEnhanceRequestPayload,
): Promise<void> {
  let error: string | null = null;

  try {
    if (!service) {
      throw new Error("Main auto-summary service is not ready");
    }
    await service.requestAutoEnhance(request.sessionId, request.mode);
  } catch (cause) {
    error = getErrorMessage(cause);
  }

  const result = {
    requestId: request.requestId,
    completed: error === null,
    error,
  } satisfies TaskAutoEnhanceResultPayload;

  try {
    await emitTo(request.sourceLabel, TASK_AUTO_ENHANCE_RESULT_EVENT, result);
  } catch {
    await emit(TASK_AUTO_ENHANCE_RESULT_EVENT, result);
  }
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
    let autoEnhanceRequestUnlisten: UnlistenFn | null = null;

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

      void (async () => {
        const service = getEnhancerService();
        if (!service) {
          return;
        }

        await handleMainEnhanceRequest(service, event.payload);
      })().catch((error) => {
        console.error("[enhancer] remote enhancement failed", error);
      });
    }).then((unlisten) => {
      if (active) {
        enhanceUnlisten = unlisten;
      } else {
        unlisten();
      }
    });

    void listen<TaskAutoEnhanceRequestPayload>(
      TASK_AUTO_ENHANCE_REQUEST_EVENT,
      (event) => {
        if (!active || !isTaskAutoEnhanceRequestPayload(event.payload)) {
          return;
        }

        void handleMainAutoEnhanceRequest(
          getEnhancerService(),
          event.payload,
        ).catch((error) => {
          console.error(
            "[enhancer] auto-summary acknowledgement failed",
            error,
          );
        });
      },
    )
      .then((unlisten) => {
        if (active) {
          autoEnhanceRequestUnlisten = unlisten;
        } else {
          unlisten();
        }
      })
      .catch((error) => {
        console.error(
          "[enhancer] auto-summary bridge failed to initialize",
          error,
        );
      });

    return () => {
      active = false;
      unsubscribe();
      syncRequestUnlisten?.();
      cancelUnlisten?.();
      enhanceUnlisten?.();
      autoEnhanceRequestUnlisten?.();
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

function isTaskAutoEnhanceRequestPayload(
  payload: unknown,
): payload is TaskAutoEnhanceRequestPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<TaskAutoEnhanceRequestPayload>;
  return (
    typeof candidate.requestId === "string" &&
    candidate.requestId.length > 0 &&
    typeof candidate.sourceLabel === "string" &&
    candidate.sourceLabel.length > 0 &&
    typeof candidate.sessionId === "string" &&
    candidate.sessionId.length > 0 &&
    (candidate.mode === "regenerate" || candidate.mode === "if_empty")
  );
}

function isTaskAutoEnhanceResultPayload(
  payload: unknown,
): payload is TaskAutoEnhanceResultPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<TaskAutoEnhanceResultPayload>;
  return (
    typeof candidate.requestId === "string" &&
    candidate.requestId.length > 0 &&
    typeof candidate.completed === "boolean" &&
    (candidate.error === null || typeof candidate.error === "string")
  );
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function toError(error: unknown) {
  return error instanceof Error ? error : new Error(String(error));
}
