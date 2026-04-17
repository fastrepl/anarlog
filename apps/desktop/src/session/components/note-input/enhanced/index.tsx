import { forwardRef } from "react";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";
import { StreamingView } from "./streaming";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
import type { NoteEditorRef } from "~/editor/session";
import { useEnhancedNoteCell } from "~/session/hooks/storage";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export const Enhanced = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    enhancedNoteId: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
  }
>(({ sessionId, enhancedNoteId, onNavigateToTitle }, ref) => {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const llmStatus = useLLMConnectionStatus();
  const { status, error } = useAITaskTask(taskId, "enhance");
  const content = useEnhancedNoteCell(enhancedNoteId, "content");

  const hasContent = content.trim().length > 0;

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

  if (status === "generating") {
    return <StreamingView enhancedNoteId={enhancedNoteId} />;
  }

  return (
    <EnhancedEditor
      ref={ref}
      sessionId={sessionId}
      enhancedNoteId={enhancedNoteId}
      onNavigateToTitle={onNavigateToTitle}
    />
  );
});
