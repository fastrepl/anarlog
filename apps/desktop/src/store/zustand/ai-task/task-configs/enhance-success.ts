import { getCurrentWindow } from "@tauri-apps/api/window";

import { commands as notificationCommands } from "@hypr/plugin-notification";
import { md2json } from "@hypr/tiptap/shared";

import { createTaskId, type TaskConfig } from ".";

import { createSummaryReadyNotificationKey } from "~/services/summary-ready-notification";

async function maybeShowSummaryReadyNotification(
  store: Parameters<
    NonNullable<TaskConfig<"enhance">["onSuccess"]>
  >[0]["store"],
  args: Parameters<NonNullable<TaskConfig<"enhance">["onSuccess"]>>[0]["args"],
) {
  try {
    const isFocused = await getCurrentWindow().isFocused();
    if (isFocused) {
      return;
    }
  } catch {
    return;
  }

  const rawNoteTitle = store.getCell(
    "enhanced_notes",
    args.enhancedNoteId,
    "title",
  );
  const noteTitle =
    typeof rawNoteTitle === "string" && rawNoteTitle.trim()
      ? rawNoteTitle.trim()
      : "Summary";
  const rawSessionTitle = store.getCell("sessions", args.sessionId, "title");
  const sessionTitle =
    typeof rawSessionTitle === "string" ? rawSessionTitle.trim() : "";

  void notificationCommands.showNotification({
    key: createSummaryReadyNotificationKey(args.sessionId, args.enhancedNoteId),
    title: `${noteTitle} ready`,
    message: sessionTitle || "Your meeting summary has been generated.",
    timeout: null,
    source: null,
    start_time: null,
    participants: null,
    event_details: null,
    action_label: "Open summary",
    options: null,
  });
}

const onSuccess: NonNullable<TaskConfig<"enhance">["onSuccess"]> = async ({
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

  await maybeShowSummaryReadyNotification(store, args);

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
