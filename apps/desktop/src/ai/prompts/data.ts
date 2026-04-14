import { useMutation, useQuery } from "@tanstack/react-query";

import { eq, promptOverrides, sql } from "@hypr/db";
import {
  commands as templateCommands,
  type EditableTemplate,
} from "@hypr/plugin-template";

import type { TaskType } from "./config";

import { db, useDrizzleLiveQuery } from "~/db";

type PromptOverrideRow = {
  task_type: string;
  content: string;
  created_at: string;
  updated_at: string;
};

const TASK_TO_EDITABLE_TEMPLATE: Record<TaskType, EditableTemplate> = {
  enhance: "enhanceUser",
  title: "titleUser",
};

export async function loadPromptOverride(
  taskType: TaskType,
): Promise<string | null> {
  const rows = await db
    .select()
    .from(promptOverrides)
    .where(eq(promptOverrides.taskType, taskType))
    .limit(1);

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
  await db
    .insert(promptOverrides)
    .values({
      taskType: params.taskType,
      content: params.content,
      createdAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
      updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
    })
    .onConflictDoUpdate({
      target: promptOverrides.taskType,
      set: {
        content: sql`excluded.content`,
        updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
      },
    });
}

async function removePromptOverride(taskType: TaskType) {
  await db
    .delete(promptOverrides)
    .where(eq(promptOverrides.taskType, taskType));
}

export function usePromptTemplateSource(taskType: TaskType) {
  return useQuery({
    queryKey: ["prompt-template-source", taskType],
    queryFn: () => fetchPromptTemplateSource(taskType),
    staleTime: Infinity,
  });
}

export function usePromptOverride(taskType: TaskType) {
  const query = db
    .select()
    .from(promptOverrides)
    .where(eq(promptOverrides.taskType, taskType))
    .limit(1);

  return useDrizzleLiveQuery<PromptOverrideRow, PromptOverrideRow | null>(
    query,
    { mapRows: (rows) => rows[0] ?? null },
  );
}

export function usePromptOverrides() {
  const query = db
    .select()
    .from(promptOverrides)
    .orderBy(promptOverrides.taskType);

  return useDrizzleLiveQuery<
    PromptOverrideRow,
    Partial<Record<TaskType, PromptOverrideRow>>
  >(query, {
    mapRows: (rows) =>
      rows.reduce<Partial<Record<TaskType, PromptOverrideRow>>>((acc, row) => {
        acc[row.task_type as TaskType] = row;
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
