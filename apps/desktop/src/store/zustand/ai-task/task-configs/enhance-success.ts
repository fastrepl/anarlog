import { md2json } from "@hypr/editor/markdown";

import { createTaskId, type TaskConfig } from ".";
import {
  appendTagLineToMarkdown,
  extractEnhanceTagNames,
  upsertSessionTags,
} from "./summary-tags";
import {
  getPersistableGeneratedTitle,
  persistGeneratedTitle,
} from "./title-success";

import { ensureMarkdownFirstLineTitle } from "~/session/title-content";
import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

type EnhanceSuccessParams = Parameters<
  NonNullable<TaskConfig<"enhance">["onSuccess"]>
>[0];

const onSuccess: NonNullable<TaskConfig<"enhance">["onSuccess"]> = async ({
  text,
  args,
  transformedArgs,
  model,
  store,
  startTask,
  getTaskState,
  signal,
}) => {
  if (!text) {
    return;
  }

  const tagNames = extractEnhanceTagNames(text, transformedArgs);
  const textWithTags = appendTagLineToMarkdown(text, tagNames);
  const currentTitle = store.getCell("sessions", args.sessionId, "title");
  let trimmedTitle =
    typeof currentTitle === "string" ? currentTitle.trim() : "";
  let generatedTitle = "";
  let shouldPersistGeneratedTitle = false;

  if (!trimmedTitle && !hasLiveSessionTitleDraft(args.sessionId)) {
    const titleTaskId = createTaskId(args.sessionId, "title");
    const titleTask = getTaskState(titleTaskId);

    if (titleTask?.status === "success") {
      generatedTitle = getPersistableGeneratedTitle(titleTask.streamedText);
    } else if (titleTask?.status !== "generating") {
      await startTask(titleTaskId, {
        model,
        taskType: "title",
        args: {
          sessionId: args.sessionId,
          enhancedNote: textWithTags,
          skipPersist: true,
        },
        onComplete: (title) => {
          generatedTitle = getPersistableGeneratedTitle(title);
        },
      });
    }

    if (signal.aborted) {
      return;
    }

    const updatedTitle = store.getCell("sessions", args.sessionId, "title");
    trimmedTitle = typeof updatedTitle === "string" ? updatedTitle.trim() : "";
    if (
      !trimmedTitle &&
      !hasLiveSessionTitleDraft(args.sessionId) &&
      generatedTitle
    ) {
      trimmedTitle = generatedTitle;
      shouldPersistGeneratedTitle = true;
    }
  }

  const didPersist = persistEnhancedNoteContent({
    text: textWithTags,
    title: trimmedTitle,
    args,
    store,
    tagNames,
  });

  if (didPersist && shouldPersistGeneratedTitle) {
    persistGeneratedTitle({
      text: generatedTitle,
      args: { sessionId: args.sessionId },
      store,
    });
  }
};

function persistEnhancedNoteContent({
  text,
  title,
  args,
  store,
  tagNames,
}: {
  text: string;
  title: string;
  args: EnhanceSuccessParams["args"];
  store: EnhanceSuccessParams["store"];
  tagNames: string[];
}): boolean {
  const titledText = ensureMarkdownFirstLineTitle(text, title);

  try {
    const jsonContent = md2json(titledText);
    store.setPartialRow("enhanced_notes", args.enhancedNoteId, {
      content: JSON.stringify(jsonContent),
    });
    upsertSessionTags(store, args.sessionId, tagNames);
    return true;
  } catch (error) {
    console.error("Failed to convert markdown to JSON:", error);
    return false;
  }
}

export const enhanceSuccess: Pick<TaskConfig<"enhance">, "onSuccess"> = {
  onSuccess,
};
