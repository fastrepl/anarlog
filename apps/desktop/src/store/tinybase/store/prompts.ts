import type { PromptStorage } from "@hypr/store";

import type { Store } from "./main";

export type TaskType = "enhance" | "title";

export function getCustomPrompt(
  store: Store,
  taskType: TaskType,
): string | null {
  const content = store.getCell("prompts", taskType, "content");
  return content || null;
}

export function setCustomPrompt(
  store: Store,
  taskType: TaskType,
  content: string,
): void {
  const userId = store.getValue("user_id");
  if (!userId) return;

  store.setRow("prompts", taskType, {
    user_id: userId,
    task_type: taskType,
    content,
  } satisfies PromptStorage);
}

export function deleteCustomPrompt(store: Store, taskType: TaskType): void {
  store.delRow("prompts", taskType);
}

export const AVAILABLE_FILTERS = ["transcript", "url"] as const;

export const TASK_CONFIGS = [
  {
    type: "enhance" as const,
    label: "Enhance Notes",
    description: "Generates structured meeting summaries from transcripts",
    variables: [
      "content",
      "session",
      "participants",
      "template",
      "pre_meeting_memo",
      "post_meeting_memo",
    ],
  },
  {
    type: "title" as const,
    label: "Title Generation",
    description: "Generates a title for the meeting note",
    variables: ["enhanced_note"],
  },
] as const;
