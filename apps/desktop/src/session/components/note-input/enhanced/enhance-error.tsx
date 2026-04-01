import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";

import { useAITask } from "~/ai/contexts";
import { useLanguageModel } from "~/ai/hooks";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export function EnhanceError({
  sessionId,
  enhancedNoteId,
  error,
}: {
  sessionId: string;
  enhancedNoteId: string;
  error: Error | undefined;
}) {
  const model = useLanguageModel("enhance");
  const generate = useAITask((state) => state.generate);
  const templateId =
    (main.UI.useCell(
      "enhanced_notes",
      enhancedNoteId,
      "template_id",
      main.STORE_ID,
    ) as string | undefined) || undefined;

  const handleRetry = () => {
    if (!model) return;

    const taskId = createTaskId(enhancedNoteId, "enhance");
    void generate(taskId, {
      model,
      taskType: "enhance",
      args: { sessionId, enhancedNoteId, templateId },
    });
  };

  return (
    <div className="flex h-full min-h-[400px] flex-col items-center justify-center gap-4">
      <AlertCircleIcon size={24} className="text-neutral-400" />
      <p className="max-w-lg text-center text-sm text-neutral-700">
        {error?.message || "Something went wrong while generating the summary."}
      </p>
      <Button
        onClick={handleRetry}
        disabled={!model}
        className="flex items-center gap-2"
        variant="default"
      >
        <RefreshCwIcon size={16} />
        <span>Retry</span>
      </Button>
    </div>
  );
}
