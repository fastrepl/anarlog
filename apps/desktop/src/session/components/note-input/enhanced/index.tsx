import { forwardRef } from "react";

import { type TiptapEditor } from "@hypr/tiptap/editor";

import { ConfigError } from "./config-error";
import { EnhancedEditor } from "./editor";
import { EnhanceError } from "./enhance-error";
import { StreamingView } from "./streaming";

import { useAITaskTask } from "~/ai/hooks";
import { useLLMConnectionStatus } from "~/ai/hooks";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export const Enhanced = forwardRef<
  { editor: TiptapEditor | null },
  { sessionId: string; enhancedNoteId: string; onNavigateToTitle?: () => void }
>(({ sessionId, enhancedNoteId, onNavigateToTitle }, ref) => {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const llmStatus = useLLMConnectionStatus();
  const { status, error } = useAITaskTask(taskId, "enhance");
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
