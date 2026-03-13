import { useEffect, useState } from "react";
import { Streamdown } from "streamdown";

import { streamdownComponents } from "@hypr/tiptap/shared";
import { cn } from "@hypr/utils";

import { useAITaskTask } from "~/ai/hooks";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { type TaskStepInfo } from "~/store/zustand/ai-task/tasks";

export function StreamingView({ enhancedNoteId }: { enhancedNoteId: string }) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const { streamedText, currentStep, isGenerating } = useAITaskTask(
    taskId,
    "enhance",
  );

  const step = currentStep as TaskStepInfo<"enhance"> | undefined;
  const hasContent = streamedText.length > 0;
  let statusText: string | null = null;
  if (isGenerating && !hasContent) {
    if (step?.type === "analyzing") {
      statusText = "Analyzing structure...";
    } else if (step?.type === "generating") {
      statusText = "Generating...";
    } else if (step?.type === "retrying") {
      statusText = `Retrying (attempt ${step.attempt})...`;
    } else {
      statusText = "Loading...";
    }
  }

  return (
    <div className="pb-2">
      {statusText ? (
        <div className="flex flex-col gap-1">
          <p className="text-sm text-neutral-500">{statusText}</p>
          <RotatingTip />
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

const TIPS = ["The Char team loves our users!"];

function RotatingTip() {
  const [index, setIndex] = useState(() =>
    Math.floor(Math.random() * TIPS.length),
  );

  useEffect(() => {
    const id = setInterval(() => {
      setIndex((i) => (i + 1) % TIPS.length);
    }, 5_000);
    return () => clearInterval(id);
  }, []);

  return <p className="pl-3 text-xs text-neutral-400">└ Tip: {TIPS[index]}</p>;
}
