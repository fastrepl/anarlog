import { md2json } from "@hypr/tiptap/shared";

import { createTaskId, type TaskConfig } from ".";

const onSuccess: NonNullable<TaskConfig<"enhance">["onSuccess"]> = ({
  text,
  args,
  model,
  store,
  startTask,
  getTaskState,
}) => {
  if (!text) {
    return;
  }

  try {
    const jsonContent = md2json(text);
    store.setPartialRow("enhanced_notes", args.enhancedNoteId, {
      content: JSON.stringify(jsonContent),
    });
  } catch (error) {
    console.error("Failed to convert markdown to JSON:", error);
    return;
  }

  const currentTitle = store.getCell("sessions", args.sessionId, "title");
  const trimmedTitle =
    typeof currentTitle === "string" ? currentTitle.trim() : "";
  if (trimmedTitle) {
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
