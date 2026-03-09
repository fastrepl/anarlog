import type { LanguageModel, TextStreamPart } from "ai";

import type {
  EnhanceSystem,
  EnhanceUser,
  TitleSystem,
  TitleUser,
} from "@hypr/plugin-template";

import { enhanceSuccess } from "./enhance-success";
import { enhanceTransform } from "./enhance-transform";
import { enhanceWorkflow } from "./enhance-workflow";
import { titleSuccess } from "./title-success";
import { titleTransform } from "./title-transform";
import { titleWorkflow } from "./title-workflow";

import type { Store as MainStore } from "~/store/tinybase/store/main";
import type { Store as SettingsStore } from "~/store/tinybase/store/settings";
import { StreamTransform } from "~/store/zustand/ai-task/shared/transform_infra";
import type { TaskState, TaskStepInfo } from "~/store/zustand/ai-task/tasks";

export type TaskType = "enhance" | "title";

export interface TaskArgsMap {
  enhance: { sessionId: string; enhancedNoteId: string; templateId?: string };
  title: { sessionId: string };
}

export interface TaskArgsMapTransformed {
  enhance: EnhanceSystem & EnhanceUser;
  title: TitleSystem & TitleUser;
}

export type TaskId<T extends TaskType = TaskType> = `${string}-${T}`;

export function createTaskId<T extends TaskType>(
  entityId: string,
  taskType: T,
): TaskId<T> {
  return `${entityId}-${taskType}` as TaskId<T>;
}

export interface TaskConfig<T extends TaskType = TaskType> {
  transformArgs: (
    args: TaskArgsMap[T],
    store: MainStore,
    settingsStore: SettingsStore,
  ) => Promise<TaskArgsMapTransformed[T]>;
  executeWorkflow: (params: {
    model: LanguageModel;
    args: TaskArgsMapTransformed[T];
    onProgress: (step: TaskStepInfo<T>) => void;
    signal: AbortSignal;
    store: MainStore;
  }) => AsyncIterable<TextStreamPart<any>>;
  transforms?: StreamTransform[];
  onSuccess?: (params: {
    taskId: TaskId<T>;
    text: string;
    model: LanguageModel;
    args: TaskArgsMap[T];
    transformedArgs: TaskArgsMapTransformed[T];
    store: MainStore;
    settingsStore: SettingsStore;
    startTask: <K extends TaskType>(
      taskId: TaskId<K>,
      config: {
        model: LanguageModel;
        taskType: K;
        args: TaskArgsMap[K];
        onComplete?: (text: string) => void;
      },
    ) => Promise<void>;
    getTaskState: <K extends TaskType>(
      taskId: TaskId<K>,
    ) => TaskState<K> | undefined;
  }) => Promise<void> | void;
}

type TaskConfigMap = {
  [K in TaskType]: TaskConfig<K>;
};

export const TASK_CONFIGS: TaskConfigMap = {
  enhance: {
    ...enhanceWorkflow,
    ...enhanceTransform,
    ...enhanceSuccess,
  },
  title: {
    ...titleWorkflow,
    ...titleTransform,
    ...titleSuccess,
  },
};
