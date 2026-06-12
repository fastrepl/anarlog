import { Streamdown } from "streamdown";

import { cn } from "@hypr/utils";

import { streamdownComponents } from "../../streamdown";

import { useAITaskTask } from "~/ai/hooks";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export function StreamingView({ enhancedNoteId }: { enhancedNoteId: string }) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const { streamedText, isGenerating } = useAITaskTask(taskId, "enhance");

  return (
    <div className="pb-2">
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
    </div>
  );
}
