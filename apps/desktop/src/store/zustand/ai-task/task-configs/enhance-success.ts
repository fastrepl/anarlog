import { md2json } from "@hypr/editor/markdown";

import { createTaskId, type TaskConfig } from ".";
import {
  appendTagLineToMarkdown,
  extractEnhanceTagNames,
  upsertSessionTags,
} from "./summary-tags";

import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

const onSuccess: NonNullable<TaskConfig<"enhance">["onSuccess"]> = ({
  text,
  args,
  transformedArgs,
  model,
  store,
  startTask,
  getTaskState,
}) => {
  if (!text) {
    return;
  }

  const tagNames = extractEnhanceTagNames(text, transformedArgs);
  const textWithTags = appendTagLineToMarkdown(text, tagNames);

  try {
    const jsonContent = md2json(textWithTags);
    store.setPartialRow("enhanced_notes", args.enhancedNoteId, {
      content: JSON.stringify(jsonContent),
    });
    upsertSessionTags(store, args.sessionId, tagNames);
  } catch (error) {
    console.error("Failed to convert markdown to JSON:", error);
    return;
  }

  const currentTitle = store.getCell("sessions", args.sessionId, "title");
  const trimmedTitle =
    typeof currentTitle === "string" ? currentTitle.trim() : "";
  if (trimmedTitle || hasLiveSessionTitleDraft(args.sessionId)) {
    return;
  }

  const titleTaskId = createTaskId(args.sessionId, "title");
  const titleTask = getTaskState(titleTaskId);
  if (titleTask?.status === "generating" || titleTask?.status === "success") {
    return;
  }

  void startTask(titleTaskId, {
    model,
    taskType: "title",
    args: { sessionId: args.sessionId },
  });
};

export const enhanceSuccess: Pick<TaskConfig<"enhance">, "onSuccess"> = {
  onSuccess,
};
