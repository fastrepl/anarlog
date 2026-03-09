import type { LanguageModel } from "ai";
import { create as mutate } from "mutative";
import type { StoreApi } from "zustand";

import { applyTransforms } from "./shared/transform_infra";
import {
  TASK_CONFIGS,
  type TaskArgsMap,
  type TaskId,
  type TaskType,
} from "./task-configs";

import type { Store as MainStore } from "~/store/tinybase/store/main";
import type { Store as SettingsStore } from "~/store/tinybase/store/settings";

export type TasksState = {
  tasks: Record<string, TaskState>;
};

export type TasksActions = {
  generate: <T extends TaskType>(
    taskId: TaskId<T>,
    config: {
      model: LanguageModel;
      taskType: T;
      args: TaskArgsMap[T];
      onComplete?: (text: string) => void;
    },
  ) => Promise<void>;
  cancel: (taskId: string) => void;
  reset: (taskId: string) => void;
  getState: <T extends TaskType>(taskId: TaskId<T>) => TaskState<T> | undefined;
};

export type TaskStepInfo<T extends TaskType = TaskType> = T extends "enhance"
  ?
      | { type: "analyzing" }
      | { type: "generating" }
      | { type: "retrying"; attempt: number; reason: string }
  : T extends "title"
    ? { type: "generating" }
    : { type: "generating" };

export type TaskStatus = "idle" | "generating" | "success" | "error";

export type TaskState<T extends TaskType = TaskType> = {
  taskType: T;
  status: TaskStatus;
  streamedText: string;
  error?: Error;
  abortController: AbortController | null;
  currentStep?: TaskStepInfo<T>;
};

export function getTaskState<T extends TaskType>(
  tasks: TasksState["tasks"],
  taskId: TaskId<T>,
): TaskState<T> | undefined {
  const state = tasks[taskId];
  if (state?.taskType) {
    return state as TaskState<T>;
  }
  return undefined;
}

const initialState: TasksState = {
  tasks: {},
};

export const createTasksSlice = <T extends TasksState & TasksActions>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
  deps: { persistedStore: MainStore; settingsStore: SettingsStore },
): TasksState & TasksActions => ({
  ...initialState,
  getState: <Task extends TaskType>(
    taskId: TaskId<Task>,
  ): TaskState<Task> | undefined => {
    const task = get().tasks[taskId];
    return task as TaskState<Task> | undefined;
  },
  cancel: (taskId: string) => {
    set((state) =>
      mutate(state, (draft) => {
        const task = draft.tasks[taskId];
        if (!task) {
          return;
        }

        task.abortController?.abort();

        draft.tasks[taskId] = {
          taskType: task.taskType,
          status: "idle",
          streamedText: task.streamedText,
          error: undefined,
          abortController: null,
          currentStep: undefined,
        };
      }),
    );
  },
  reset: (taskId: string) => {
    const state = get().tasks[taskId];
    if (state) {
      set((currentState) =>
        mutate(currentState, (draft) => {
          draft.tasks[taskId] = {
            taskType: state.taskType,
            status: "idle",
            streamedText: "",
            error: undefined,
            abortController: null,
            currentStep: undefined,
          };
        }),
      );
    }
  },
  generate: async <Task extends TaskType>(
    taskId: TaskId<Task>,
    config: {
      model: LanguageModel;
      taskType: Task;
      args: TaskArgsMap[Task];
      onComplete?: (text: string) => void;
    },
  ) => {
    const existingTask = get().tasks[taskId];
    if (existingTask?.status === "generating") {
      return;
    }

    const abortController = new AbortController();
    const taskConfig = TASK_CONFIGS[config.taskType];

    try {
      set((state) =>
        mutate(state, (draft) => {
          draft.tasks[taskId] = {
            taskType: config.taskType,
            status: "generating",
            streamedText: "",
            error: undefined,
            abortController,
            currentStep: undefined,
          };
        }),
      );

      const enrichedArgs = await taskConfig.transformArgs(
        config.args,
        deps.persistedStore,
        deps.settingsStore,
      );
      let fullText = "";

      const checkAbort = () => {
        if (abortController.signal.aborted) {
          const error = new Error("Aborted");
          error.name = "AbortError";
          throw error;
        }
      };

      const onProgress = (step: TaskStepInfo<Task>) => {
        set((state) =>
          mutate(state, (draft) => {
            const currentState = draft.tasks[taskId];
            if (currentState?.taskType === config.taskType) {
              (currentState as any).currentStep = step;
            }
          }),
        );
      };

      const workflowStream = taskConfig.executeWorkflow({
        model: config.model,
        args: enrichedArgs,
        onProgress,
        signal: abortController.signal,
        store: deps.persistedStore,
      });

      const transforms = taskConfig.transforms ?? [];
      const transformedStream = applyTransforms(workflowStream, transforms, {
        stopStream: () => abortController.abort(),
      });

      for await (const chunk of transformedStream) {
        checkAbort();

        if (chunk.type === "error") {
          throw chunk.error;
        } else if (chunk.type === "text-delta") {
          fullText += chunk.text;

          set((state) =>
            mutate(state, (draft) => {
              const currentState = draft.tasks[taskId];
              if (currentState) {
                currentState.streamedText = fullText;
              }
            }),
          );
        }
      }

      set((state) =>
        mutate(state, (draft) => {
          draft.tasks[taskId] = {
            taskType: config.taskType,
            status: "success",
            streamedText: fullText,
            error: undefined,
            abortController: null,
            currentStep: undefined,
          };
        }),
      );

      try {
        await taskConfig.onSuccess?.({
          taskId,
          text: fullText,
          model: config.model,
          args: config.args,
          transformedArgs: enrichedArgs,
          store: deps.persistedStore,
          settingsStore: deps.settingsStore,
          startTask: (nextTaskId, nextConfig) =>
            get().generate(nextTaskId, nextConfig),
          getTaskState: (nextTaskId) => getTaskState(get().tasks, nextTaskId),
        });
      } catch (error) {
        console.error("Task post-success hook failed:", error);
      }

      try {
        config.onComplete?.(fullText);
      } catch (error) {
        console.error("Task onComplete callback failed:", error);
      }
    } catch (err) {
      if (
        err instanceof Error &&
        (err.name === "AbortError" || err.message === "Aborted")
      ) {
        set((state) =>
          mutate(state, (draft) => {
            draft.tasks[taskId] = {
              taskType: config.taskType,
              status: "idle",
              streamedText: "",
              error: undefined,
              abortController: null,
              currentStep: undefined,
            };
          }),
        );
      } else {
        const error = extractUnderlyingError(err);
        set((state) =>
          mutate(state, (draft) => {
            draft.tasks[taskId] = {
              taskType: config.taskType,
              status: "error",
              streamedText: "",
              error,
              abortController: null,
              currentStep: undefined,
            };
          }),
        );
      }
    }
  },
});

function extractUnderlyingError(err: unknown): Error {
  if (!(err instanceof Error)) {
    return new Error(String(err));
  }

  if (err.name === "AI_RetryError") {
    if ("cause" in err && err.cause instanceof Error) {
      return err.cause;
    }

    if ("lastError" in err && err.lastError instanceof Error) {
      return err.lastError;
    }

    if ("errors" in err && Array.isArray((err as any).errors)) {
      const errors = (err as any).errors;
      if (errors.length > 0 && errors[errors.length - 1] instanceof Error) {
        return errors[errors.length - 1];
      }
    }

    const match = err.message.match(/Last error: (.+)$/);
    if (match) {
      const underlyingMessage = match[1];
      const underlyingError = new Error(underlyingMessage);
      underlyingError.name = "AI_ProviderError";
      return underlyingError;
    }
  }

  return err;
}
