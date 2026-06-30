import { Trans } from "@lingui/react/macro";
import { Streamdown } from "streamdown";

import { cn } from "@hypr/utils";

import { streamdownComponents } from "../../streamdown";

import { useAITaskTask } from "~/ai/hooks";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

function SummaryTitleSpace() {
  return (
    <div
      data-testid="summary-title-space"
      className="pointer-events-none mb-3 flex h-[1.875rem] items-center"
    >
      <span
        aria-hidden="true"
        className="text-muted-foreground animate-pulse text-[1.5rem] leading-[1.875rem] font-semibold opacity-60"
      >
        <Trans>Generating title...</Trans>
      </span>
    </div>
  );
}

export function StreamingView({
  sessionId,
  enhancedNoteId,
}: {
  sessionId: string;
  enhancedNoteId: string;
}) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const { streamedText, isGenerating } = useAITaskTask(taskId, "enhance");
  const sessionTitle = main.UI.useCell(
    "sessions",
    sessionId,
    "title",
    main.STORE_ID,
  );
  const shouldGenerateTitle =
    typeof sessionTitle !== "string" || sessionTitle.trim().length === 0;

  if (streamedText.trim().length === 0) {
    return (
      <div
        role="status"
        aria-live="polite"
        className="text-muted-foreground flex flex-col gap-0.5 pb-2 text-sm"
      >
        <p className="leading-5">
          <Trans>Analyzing structure...</Trans>
        </p>
        <p className="flex items-start gap-1.5 pl-4 text-xs leading-5">
          <span
            aria-hidden="true"
            className="border-muted-foreground/60 mt-[5px] h-2 w-2 shrink-0 rounded-bl-[2px] border-b border-l"
          />
          <span>
            <Trans>Tip: The Anarlog team loves our users!</Trans>
          </span>
        </p>
      </div>
    );
  }

  return (
    <div className="pb-2">
      <div className="flex flex-col gap-1">
        {shouldGenerateTitle ? <SummaryTitleSpace /> : null}
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
