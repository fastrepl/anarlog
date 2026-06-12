import type { EditorView } from "prosemirror-view";
import { forwardRef } from "react";

import type { NoteEditorRef } from "@hypr/editor/note";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";
import { StreamingView } from "./streaming";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
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
    const { status, error, hasTask } = useAITaskTask(taskId, "enhance");
    const content = main.UI.useCell(
      "enhanced_notes",
      enhancedNoteId,
      "content",
      main.STORE_ID,
    );

    const hasContent = typeof content === "string" && content.trim().length > 0;

    const isConfigError =
      llmStatus.status === "pending" ||
      (llmStatus.status === "error" &&
        (llmStatus.reason === "missing_config" ||
          llmStatus.reason === "not_pro" ||
          llmStatus.reason === "unauthenticated"));

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

    if (
      status === "generating" ||
      (!hasContent && status === "idle" && !hasTask)
    ) {
      return (
        <StreamingView
          enhancedNoteId={enhancedNoteId}
          pending={status === "idle"}
        />
      );
    }

    return (
      <EnhancedEditor
        ref={ref}
        sessionId={sessionId}
        enhancedNoteId={enhancedNoteId}
        onNavigateToTitle={onNavigateToTitle}
        onViewReady={onViewReady}
        onViewDisposed={onViewDisposed}
      />
    );
  },
);
