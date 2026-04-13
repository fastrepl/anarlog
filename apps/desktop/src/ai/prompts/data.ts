import { useMutation, useQuery } from "@tanstack/react-query";

import { execute } from "@hypr/plugin-db";
import {
  commands as templateCommands,
  type EditableTemplate,
} from "@hypr/plugin-template";

import type { TaskType } from "./config";

import { useLiveQuery } from "~/db/use-live-query";

export type PromptOverrideRow = {
  task_type: TaskType;
  content: string;
  created_at: string;
  updated_at: string;
};

const PROMPT_OVERRIDE_COLUMNS = "task_type, content, created_at, updated_at";

const TASK_TO_EDITABLE_TEMPLATE: Record<TaskType, EditableTemplate> = {
  enhance: "enhanceUser",
  title: "titleUser",
};

export async function loadPromptOverride(
  taskType: TaskType,
): Promise<string | null> {
  const rows = await execute<PromptOverrideRow>(
    `SELECT ${PROMPT_OVERRIDE_COLUMNS} FROM prompt_overrides WHERE task_type = ? LIMIT 1`,
    [taskType],
  );

  return rows[0]?.content ?? null;
}

async function fetchPromptTemplateSource(taskType: TaskType): Promise<string> {
  const result = await templateCommands.getTemplateSource(
    TASK_TO_EDITABLE_TEMPLATE[taskType],
  );

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}

async function upsertPromptOverride(params: {
  taskType: TaskType;
  content: string;
}) {
  await execute(
    `INSERT INTO prompt_overrides (task_type, content, updated_at)
     VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
     ON CONFLICT(task_type) DO UPDATE SET
       content = excluded.content,
       updated_at = excluded.updated_at`,
    [params.taskType, params.content],
  );
}

async function removePromptOverride(taskType: TaskType) {
  await execute("DELETE FROM prompt_overrides WHERE task_type = ?", [taskType]);
}

export function usePromptTemplateSource(taskType: TaskType) {
  return useQuery({
    queryKey: ["prompt-template-source", taskType],
    queryFn: () => fetchPromptTemplateSource(taskType),
    staleTime: Infinity,
  });
}

export function usePromptOverride(taskType: TaskType) {
  return useLiveQuery<PromptOverrideRow, PromptOverrideRow | null>({
    sql: `SELECT ${PROMPT_OVERRIDE_COLUMNS} FROM prompt_overrides WHERE task_type = ? LIMIT 1`,
    params: [taskType],
    mapRows: (rows) => rows[0] ?? null,
  });
}

export function usePromptOverrides() {
  return useLiveQuery<
    PromptOverrideRow,
    Partial<Record<TaskType, PromptOverrideRow>>
  >({
    sql: `SELECT ${PROMPT_OVERRIDE_COLUMNS} FROM prompt_overrides ORDER BY task_type`,
    mapRows: (rows) =>
      rows.reduce<Partial<Record<TaskType, PromptOverrideRow>>>((acc, row) => {
        acc[row.task_type] = row;
        return acc;
      }, {}),
  });
}

export function useUpsertPromptOverrideMutation(taskType: TaskType) {
  return useMutation({
    mutationFn: async (content: string) => {
      await upsertPromptOverride({ taskType, content });
    },
  });
}

export function useDeletePromptOverrideMutation(taskType: TaskType) {
  return useMutation({
    mutationFn: async () => {
      await removePromptOverride(taskType);
    },
  });
}
