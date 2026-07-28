import { md2json } from "@hypr/editor/markdown";
import { beginCloudsyncActivity } from "@hypr/plugin-db";
import { commands as localApiCommands } from "@hypr/plugin-local-api";

import { createTaskId, type TaskConfig } from ".";
import {
  appendTagLineToMarkdown,
  extractEnhanceTagNames,
} from "./summary-tags";
import {
  getPersistableGeneratedTitle,
  persistGeneratedTitle,
} from "./title-success";

import { syncCloudApiSnapshotBestEffort } from "~/cloud-api/client";
import { releaseCloudsyncActivityEventually } from "~/db/cloudsync-activity";
import { retryDatabaseLock } from "~/db/retry";
import { inferAutomaticSpeakerAssignments } from "~/services/enhancer/speaker-attribution";
import {
  constrainSummaryLength,
  countNormalizedCharacters,
  getSummaryLengthPolicy,
} from "~/services/enhancer/summary-length";
import {
  persistGeneratedEnhancedNote,
  type TranscriptSpeakerHintsUpdate,
} from "~/session/content-mutations";
import { loadSessionContentSnapshot } from "~/session/content-queries";
import { ensureMarkdownFirstLineTitle } from "~/session/title-content";
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
  const lengthPolicy = getSummaryLengthPolicy(transformedArgs.transcripts);
  const constrainedText = constrainSummaryLength(text, lengthPolicy);
  if (!constrainedText) {
    return;
  }

  const cloudsyncLeaseKey = `${taskId}:${id()}`;
  const tagNames = extractEnhanceTagNames(constrainedText, transformedArgs);
  const textWithTags = appendTagLineToMarkdown(constrainedText, tagNames);
  const initialSnapshot = await loadSessionContentSnapshot(args.sessionId);
  if (!initialSnapshot) {
    throw new Error(`Session ${args.sessionId} no longer exists`);
  }

  let trimmedTitle = initialSnapshot.title.trim();
  let generatedTitle = "";
  let shouldPersistGeneratedTitle = false;
  let transcriptSpeakerHints: TranscriptSpeakerHintsUpdate[] = [];

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

  try {
    transcriptSpeakerHints = await inferAutomaticSpeakerAssignments({
      generatedSummary: constrainedText,
      model,
      snapshot: initialSnapshot,
      signal,
    });
  } catch (error) {
    if (signal.aborted) {
      return;
    }
    console.error("[enhance] failed to identify transcript speakers", error);
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

    const titledText = ensureMarkdownFirstLineTitle(
      constrainedText,
      trimmedTitle,
    );
    const tagLine = appendTagLineToMarkdown("", tagNames);
    const reservedTagCharacters = tagLine
      ? countNormalizedCharacters(tagLine) + 1
      : 0;
    const persistableBody = constrainSummaryLength(
      titledText,
      lengthPolicy
        ? {
            ...lengthPolicy,
            maxCharacters: Math.max(
              0,
              lengthPolicy.maxCharacters - reservedTagCharacters,
            ),
            maxSections: null,
          }
        : null,
    );
    // A reset/regenerate aborts this run; a stale run that persisted anyway
    // would overwrite the replacement's summary with old content.
    if (signal.aborted) {
      return;
    }

    const persistableText = appendTagLineToMarkdown(persistableBody, tagNames);
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
        ...(transcriptSpeakerHints.length > 0
          ? { transcriptSpeakerHints }
          : {}),
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
      syncCloudApiSnapshotBestEffort(args.sessionId);
    }
  } finally {
    await releaseCloudsyncActivityEventually("enhance", cloudsyncLeaseKey);
  }
};

export const enhanceSuccess: Pick<TaskConfig<"enhance">, "onSuccess"> = {
  onSuccess: runEnhanceSuccess,
};
