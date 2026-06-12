import { Streamdown } from "streamdown";

import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { streamdownComponents } from "../../streamdown";

import { useAITaskTask } from "~/ai/hooks";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { type TaskStepInfo } from "~/store/zustand/ai-task/tasks";

export function StreamingView({
  enhancedNoteId,
  pending = false,
}: {
  enhancedNoteId: string;
  pending?: boolean;
}) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const { streamedText, currentStep, isGenerating } = useAITaskTask(
    taskId,
    "enhance",
  );

  const step = currentStep as TaskStepInfo<"enhance"> | undefined;
  const hasContent = streamedText.trim().length > 0;
  let statusText: string | null = null;
  if (isGenerating && !hasContent) {
    if (step?.type === "analyzing") {
      statusText = "Analyzing transcript...";
    } else if (step?.type === "generating") {
      statusText = "Writing summary...";
    } else if (step?.type === "retrying") {
      statusText = `Retrying (attempt ${step.attempt})...`;
    } else {
      statusText = "Preparing summary...";
    }
  } else if (pending && !hasContent) {
    statusText = "Preparing summary...";
  }

  return (
    <div className="pb-2">
      {statusText ? (
        <div
          role="status"
          aria-live="polite"
          className="flex min-h-[260px] flex-col items-center justify-center gap-2 text-center"
        >
          <Spinner size={16} />
          <div className="flex flex-col gap-1">
            <p className="text-foreground text-sm font-medium">{statusText}</p>
            <p className="text-muted-foreground text-xs">
              The summary will appear here as soon as it starts streaming.
            </p>
            <p className="text-muted-foreground text-xs">
              Tip: The Anarlog team loves our users!
            </p>
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-1">
          <Streamdown
            components={streamdownComponents}
            className={cn(["flex flex-col"])}
            caret="block"
            isAnimating={isGenerating}
          >
            {streamedText}
          </Streamdown>
        </div>
      )}
    </div>
  );
}
