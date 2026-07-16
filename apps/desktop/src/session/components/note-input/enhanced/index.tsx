import type { EditorView } from "prosemirror-view";
import { forwardRef } from "react";

import type { NoteEditorRef } from "@hypr/editor/note";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";
import { StreamingView } from "./streaming";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
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

    if (status === "error") {
      return (
        <EnhanceError
          sessionId={sessionId}
          enhancedNoteId={enhancedNoteId}
          error={error}
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
      ) : null;
    }

    const isConfigError = shouldShowEmptySummaryConfigError(llmStatus);

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
