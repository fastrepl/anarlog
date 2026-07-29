import type { LanguageModel, TextStreamPart } from "ai";

import type {
  EnhanceSystem,
  EnhanceUser,
  TitleSystem,
  TitleUser,
} from "@anlg/plugin-template";

import type { EnhanceImageContext } from "./enhance-images";
import { enhanceSuccess, runEnhanceSuccess } from "./enhance-success";
import { enhanceTransform } from "./enhance-transform";
import { enhanceWorkflow } from "./enhance-workflow";
import { titleSuccess } from "./title-success";
import { titleTransform } from "./title-transform";
import { titleWorkflow } from "./title-workflow";

import { trackMeetingNoteCompletion } from "~/onboarding/meeting-note-analytics";
import type { SettingValues } from "~/settings/schema";
import { StreamTransform } from "~/store/zustand/ai-task/shared/transform_infra";
import type { TaskState, TaskStepInfo } from "~/store/zustand/ai-task/tasks";

export type TaskType = "enhance" | "title";

export interface TaskArgsMap {
  enhance: {
    sessionId: string;
    enhancedNoteId: string;
    templateId?: string;
    pendingAutoEnhance?: {
      generation: string;
      expectedBody: string;
      expectedContentFormat: string;
    };
  };
  title: {
    sessionId: string;
    enhancedNote?: string;
    skipPersist?: boolean;
  };
}

export interface TaskArgsMapTransformed {
  enhance: EnhanceSystem &
    EnhanceUser & {
      imageContext: EnhanceImageContext[];
    };
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
    settingsValues: SettingValues,
  ) => Promise<TaskArgsMapTransformed[T]>;
  executeWorkflow: (params: {
    model: LanguageModel;
    args: TaskArgsMapTransformed[T];
    onProgress: (step: TaskStepInfo<T>) => void;
    signal: AbortSignal;
  }) => AsyncIterable<TextStreamPart<any>>;
  transforms?: StreamTransform[];
  onSuccess?: (params: {
    taskId: TaskId<T>;
    text: string;
    model: LanguageModel;
    args: TaskArgsMap[T];
    transformedArgs: TaskArgsMapTransformed[T];
    signal: AbortSignal;
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

const onEnhanceSuccess: NonNullable<
  TaskConfig<"enhance">["onSuccess"]
> = async (params) => {
  await runEnhanceSuccess({
    ...params,
    onPersisted: () => {
      void trackMeetingNoteCompletion(params.args.sessionId).catch((error) => {
        console.error(
          "[analytics] failed to record meeting note completion",
          error,
        );
      });
    },
  });
};

export const TASK_CONFIGS: TaskConfigMap = {
  enhance: {
    ...enhanceWorkflow,
    ...enhanceTransform,
    ...enhanceSuccess,
    onSuccess: onEnhanceSuccess,
  },
  title: {
    ...titleWorkflow,
    ...titleTransform,
    ...titleSuccess,
  },
};
