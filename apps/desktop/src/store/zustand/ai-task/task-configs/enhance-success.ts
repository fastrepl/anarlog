import { md2json } from "@anlg/editor/markdown";
import { beginCloudsyncActivity } from "@anlg/plugin-db";
import { commands as localApiCommands } from "@anlg/plugin-local-api";
import { commands as templateCommands } from "@anlg/plugin-template";

import { createTaskId, type TaskConfig } from ".";
import {
  getPersistableGeneratedTitle,
  persistGeneratedTitle,
} from "./title-success";

import { runNoteEnhancedAutomations } from "~/automations/engine";
import { syncCloudApiSnapshotBestEffort } from "~/cloud-api/client";
import { releaseCloudsyncActivityEventually } from "~/db/cloudsync-activity";
import { retryDatabaseLock } from "~/db/retry";
import { showSummaryReadyNotification } from "~/services/enhancer/summary-notification";
import { persistGeneratedEnhancedNote } from "~/session/content-mutations";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import { requestAppAttention } from "~/shared/app-attention";
import { playCompletionSound } from "~/shared/completion-sound";
import { id } from "~/shared/utils";
import { hasLiveSessionTitleDraft } from "~/store/zustand/live-title";

type EnhanceSuccessParams = Parameters<
  NonNullable<TaskConfig<"enhance">["onSuccess"]>
>[0] & {
  onPersisted?: () => void;
};

export const runEnhanceSuccess = async ({
  text,
  taskId,
  args,
  transformedArgs,
  model,
  startTask,
  getTaskState,
  signal,
  onPersisted,
}: EnhanceSuccessParams) => {
  const lengthPolicy = transformedArgs.template?.sections.length
    ? null
    : transformedArgs.lengthPolicy;
  const preparedResult = await templateCommands.prepareGeneratedSummary({
    text,
    length_policy: lengthPolicy,
    tag_sources: [
      transformedArgs.preMeetingMemo,
      transformedArgs.postMeetingMemo,
      transformedArgs.template?.title,
      transformedArgs.template?.description,
      ...(transformedArgs.template?.sections ?? []).flatMap((section) => [
        section.title,
        section.description,
      ]),
    ].filter((source): source is string => typeof source === "string"),
  });
  if (preparedResult.status === "error") {
    throw new Error(preparedResult.error);
  }
  if (!preparedResult.data) {
    return;
  }

  const { constrained_text: constrainedText, tag_names: tagNames } =
    preparedResult.data;
  const textWithTags = preparedResult.data.text_with_tags;
  const cloudsyncLeaseKey = `${taskId}:${id()}`;
  const initialSnapshot = await loadSessionContentSnapshot(args.sessionId);
  if (!initialSnapshot) {
    throw new Error(`Session ${args.sessionId} no longer exists`);
  }

  let trimmedTitle = initialSnapshot.title.trim();
  let generatedTitle = "";
  let shouldPersistGeneratedTitle = false;

  if (!trimmedTitle && !hasLiveSessionTitleDraft(args.sessionId)) {
    const titleTaskId = createTaskId(args.sessionId, "title");
    const titleTask = getTaskState(titleTaskId);

    if (titleTask?.status === "success" || titleTask?.status === "generating") {
      generatedTitle = getPersistableGeneratedTitle(titleTask.streamedText);
    } else {
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
  }

  await beginCloudsyncActivity("enhance", cloudsyncLeaseKey);
  try {
    const snapshot = await loadSessionContentSnapshot(args.sessionId);
    if (!snapshot) {
      throw new Error(`Session ${args.sessionId} no longer exists`);
    }
    const note = snapshot.enhancedNotes.find(
      (candidate) => candidate.id === args.enhancedNoteId,
    );
    if (!note) {
      throw new Error(`Summary ${args.enhancedNoteId} no longer exists`);
    }

    trimmedTitle = snapshot.title.trim();
    if (
      !trimmedTitle &&
      !hasLiveSessionTitleDraft(args.sessionId) &&
      generatedTitle
    ) {
      trimmedTitle = generatedTitle;
      shouldPersistGeneratedTitle = true;
    }

    const composedResult = await templateCommands.composeGeneratedSummary({
      constrained_text: constrainedText,
      title: trimmedTitle || null,
      tag_names: tagNames,
      length_policy: lengthPolicy,
    });
    if (composedResult.status === "error") {
      throw new Error(composedResult.error);
    }
    // A reset/regenerate aborts this run; a stale run that persisted anyway
    // would overwrite the replacement's summary with old content.
    if (signal.aborted) {
      return;
    }

    const persistableText = composedResult.data;
    await retryDatabaseLock(() => {
      if (signal.aborted) {
        return Promise.resolve();
      }

      return persistGeneratedEnhancedNote({
        sessionId: args.sessionId,
        ownerUserId: snapshot.ownerUserId,
        note: {
          id: note.id,
          currentContent: args.pendingAutoEnhance?.expectedBody ?? note.content,
          currentContentFormat:
            args.pendingAutoEnhance?.expectedContentFormat ??
            note.contentFormat,
          nextContent: JSON.stringify(md2json(persistableText)),
        },
        tagNames,
        ...(args.pendingAutoEnhance
          ? { pendingAutoEnhance: args.pendingAutoEnhance }
          : {}),
      }).then(onPersisted);
    });

    if (shouldPersistGeneratedTitle && !signal.aborted) {
      await persistGeneratedTitle({
        text: generatedTitle,
        args: { sessionId: args.sessionId },
      });
    }

    if (!signal.aborted) {
      void localApiCommands.dispatchEvent("note.enhanced", args.sessionId);
      void runNoteEnhancedAutomations(args.sessionId);
      syncCloudApiSnapshotBestEffort(args.sessionId);
      void showSummaryReadyNotification(args.sessionId, trimmedTitle);
      void playCompletionSound();
      void requestAppAttention();
    }
  } finally {
    await releaseCloudsyncActivityEventually("enhance", cloudsyncLeaseKey);
  }
};

export const enhanceSuccess: Pick<TaskConfig<"enhance">, "onSuccess"> = {
  onSuccess: runEnhanceSuccess,
};
