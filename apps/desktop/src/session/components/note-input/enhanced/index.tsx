import { Trans } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import type { EditorView } from "prosemirror-view";
import { forwardRef } from "react";

import type { NoteEditorRef } from "@anlg/editor/note";
import { Button } from "@anlg/ui/components/ui/button";
import { defaultSummaryDocumentId } from "@anlg/utils/session";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";
import { StreamingView } from "./streaming";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
import {
  isMainAITaskHostWindow,
  requestMainEnhance,
} from "~/ai/task-window-sync";
import { getEnhancerService } from "~/services/enhancer";
import { hasStoredNoteContent } from "~/session/components/shared";
import { shouldShowEmptySummaryConfigError } from "~/session/enhance-config";
import { useEnhancedNote } from "~/session/queries";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export const Enhanced = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    sessionTitle: string;
    enhancedNoteId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    onViewReady?: (view: EditorView) => void;
    onViewDisposed?: (view: EditorView) => void;
  }
>(
  (
    {
      sessionId,
      sessionTitle,
      enhancedNoteId,
      onNavigateToTitle,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const taskId = createTaskId(enhancedNoteId, "enhance");
    const llmStatus = useLLMConnectionStatus();
    const { status, error, streamedText } = useAITaskTask(taskId, "enhance");
    const enhancedNote = useEnhancedNote(enhancedNoteId);
    const content = enhancedNote?.content;

    const hasContent = hasStoredNoteContent(content);
    const isAwaitingPersistedContent =
      status === "success" && streamedText.trim().length > 0 && !hasContent;
    const showStreaming = status === "generating" || isAwaitingPersistedContent;
    const isConfigError = shouldShowEmptySummaryConfigError(llmStatus);

    if (status === "error") {
      return (
        <EnhanceError
          sessionId={sessionId}
          enhancedNoteId={enhancedNoteId}
          error={error}
          isUnauthenticated={
            llmStatus.status === "error" &&
            llmStatus.reason === "unauthenticated"
          }
        />
      );
    }

    if (!enhancedNote) {
      return showStreaming ? (
        <StreamingView
          sessionId={sessionId}
          sessionTitle={sessionTitle}
          enhancedNoteId={enhancedNoteId}
        />
      ) : enhancedNoteId === defaultSummaryDocumentId(sessionId) ? (
        isConfigError ? (
          <ConfigError />
        ) : (
          <EmptySummary sessionId={sessionId} />
        )
      ) : null;
    }

    if (status === "idle" && isConfigError && !hasContent) {
      return <ConfigError />;
    }

    if (showStreaming) {
      return (
        <StreamingView
          sessionId={sessionId}
          sessionTitle={sessionTitle}
          enhancedNoteId={enhancedNoteId}
        />
      );
    }

    return (
      <EnhancedEditor
        ref={ref}
        sessionId={sessionId}
        sessionTitle={sessionTitle}
        enhancedNoteId={enhancedNoteId}
        content={enhancedNote.content}
        onNavigateToTitle={onNavigateToTitle}
        onViewReady={onViewReady}
        onViewDisposed={onViewDisposed}
      />
    );
  },
);

function EmptySummary({ sessionId }: { sessionId: string }) {
  const generate = useMutation({
    mutationFn: async () => {
      if (!isMainAITaskHostWindow()) {
        await requestMainEnhance(sessionId);
        return;
      }
      const service = getEnhancerService();
      if (!service) throw new Error("Summary generation is not ready yet.");
      const result = await service.enhance(sessionId);
      if (result.type === "no_model")
        throw new Error("Set up AI summaries first.");
      if (result.type === "too_short")
        throw new Error("Not enough transcript recorded to summarize.");
    },
  });
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 px-6">
      <Button onClick={() => generate.mutate()} disabled={generate.isPending}>
        <Trans>Generate summary</Trans>
      </Button>
      {generate.error && <p role="alert">{generate.error.message}</p>}
    </div>
  );
}
