import type { EditorView } from "prosemirror-view";
import { forwardRef, useMemo } from "react";

import { md2json } from "@hypr/editor/markdown";
import type { NoteEditorRef } from "@hypr/editor/note";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
import { shouldShowEmptySummaryConfigError } from "~/session/enhance-config";
import * as main from "~/store/tinybase/store/main";
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
    const { status, error, streamedText } = useAITaskTask(taskId, "enhance");
    const content = main.UI.useCell(
      "enhanced_notes",
      enhancedNoteId,
      "content",
      main.STORE_ID,
    );
    const streamingPreviewContent = useMemo(
      () => (status === "generating" ? md2json(streamedText) : undefined),
      [status, streamedText],
    );

    const hasContent = typeof content === "string" && content.trim().length > 0;

    const isConfigError = shouldShowEmptySummaryConfigError(llmStatus);

    if (status === "idle" && isConfigError && !hasContent) {
      return <ConfigError status={llmStatus} />;
    }

    if (status === "error") {
      return (
        <EnhanceError
          sessionId={sessionId}
          enhancedNoteId={enhancedNoteId}
          error={error}
        />
      );
    }

    return (
      <EnhancedEditor
        ref={ref}
        sessionId={sessionId}
        enhancedNoteId={enhancedNoteId}
        contentOverride={streamingPreviewContent}
        onNavigateToTitle={onNavigateToTitle}
        onViewReady={onViewReady}
        onViewDisposed={onViewDisposed}
      />
    );
  },
);
