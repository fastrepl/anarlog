import type { LanguageModel } from "ai";
import { useCallback, useEffect, useRef } from "react";
import { shallow } from "zustand/shallow";

import { useAITask } from "../contexts";

import { useLatestRef } from "~/shared/hooks/useLatestRef";
import type {
  TaskArgsMap,
  TaskId,
  TaskType,
} from "~/store/zustand/ai-task/task-configs";
import {
  getTaskState,
  type TaskState,
  type TaskStatus,
} from "~/store/zustand/ai-task/tasks";

type SuccessPayload<T extends TaskType> = {
  text: string;
  task: TaskState<T>;
};

type ErrorPayload<T extends TaskType> = {
  error: Error | undefined;
  task: TaskState<T> | undefined;
};

type Options<T extends TaskType> = {
  onSuccess?: (payload: SuccessPayload<T>) => void;
  onError?: (payload: ErrorPayload<T>) => void;
};

type StartParams<T extends TaskType> = {
  model: LanguageModel;
  args: TaskArgsMap[T];
  onComplete?: (text: string) => void;
};

export function useAITaskTask<T extends TaskType>(
  taskId: TaskId<T> | null,
  taskType: T,
  options?: Options<T>,
) {
  const enabled = taskId !== null;

  const { taskState, generate, cancel, reset } = useAITask(
    useCallback(
      (state) => ({
        taskState: enabled ? getTaskState(state.tasks, taskId) : undefined,
        generate: state.generate,
        cancel: state.cancel,
        reset: state.reset,
      }),
      [taskId, enabled],
    ),
    shallow,
  );

  const status = taskState?.status ?? "idle";
  const streamedText = taskState?.streamedText ?? "";
  const error = taskState?.error;
  const currentStep = taskState?.currentStep;

  const callbacksRef = useLatestRef(options);
  const prevStatusRef = useRef<TaskStatus>(status);

  useEffect(() => {
    const prevStatus = prevStatusRef.current;
    if (prevStatus === "generating") {
      if (status === "success" && taskState) {
        callbacksRef.current?.onSuccess?.({
          text: streamedText,
          task: taskState,
        });
      } else if (status === "error") {
        callbacksRef.current?.onError?.({
          error,
          task: taskState,
        });
      }
    }
    prevStatusRef.current = status;
  }, [status, streamedText, error, taskState, callbacksRef]);

  const start = useCallback(
    (config: StartParams<T>) => {
      if (!enabled) return Promise.resolve();
      return generate(taskId, { ...config, taskType });
    },
    [generate, taskId, taskType, enabled],
  );

  const cancelTask = useCallback(() => {
    if (!enabled) return;
    cancel(taskId);
  }, [cancel, taskId, enabled]);

  const resetTask = useCallback(() => {
    if (!enabled) return;
    reset(taskId);
  }, [reset, taskId, enabled]);

  return {
    status,
    streamedText,
    error,
    currentStep,
    isGenerating: status === "generating",
    isSuccess: status === "success",
    isError: status === "error",
    isIdle: status === "idle",
    start,
    cancel: cancelTask,
    reset: resetTask,
  };
}
