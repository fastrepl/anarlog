import type { EditorView } from "prosemirror-view";
import { forwardRef } from "react";

import type { NoteEditorRef } from "@hypr/editor/note";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";
import { StreamingView } from "./streaming";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
import { shouldShowEmptySummaryConfigError } from "~/session/enhance-config";
import { useEnhancedNote } from "~/session/queries";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export const Enhanced = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    enhancedNoteId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    onViewReady?: (view: EditorView) => void;
    onViewDisposed?: (view: EditorView) => void;
  }
>(
  (
    {
      sessionId,
      enhancedNoteId,
      onNavigateToTitle,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const taskId = createTaskId(enhancedNoteId, "enhance");
    const llmStatus = useLLMConnectionStatus();
    const { status, error } = useAITaskTask(taskId, "enhance");
    const enhancedNote = useEnhancedNote(enhancedNoteId);
    const content = enhancedNote?.content;

    const hasContent = typeof content === "string" && content.trim().length > 0;

    if (status === "error") {
      return (
        <EnhanceError
          sessionId={sessionId}
          enhancedNoteId={enhancedNoteId}
          error={error}
        />
      );
    }

    if (status === "generating") {
      return (
        <StreamingView sessionId={sessionId} enhancedNoteId={enhancedNoteId} />
      );
    }

    if (!enhancedNote) {
      return null;
    }

    const isConfigError = shouldShowEmptySummaryConfigError(llmStatus);

    if (status === "idle" && isConfigError && !hasContent) {
      return <ConfigError status={llmStatus} />;
    }

    return (
      <EnhancedEditor
        ref={ref}
        sessionId={sessionId}
        enhancedNoteId={enhancedNoteId}
        content={enhancedNote.content}
        onNavigateToTitle={onNavigateToTitle}
        onViewReady={onViewReady}
        onViewDisposed={onViewDisposed}
      />
    );
  },
);
