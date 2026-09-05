import { Trans } from "@lingui/react/macro";
import { Streamdown } from "streamdown";

import { streamdownIcons } from "@anlg/ui/components/streamdown-icons";
import { cn } from "@anlg/utils";

import { streamdownComponents } from "../../streamdown";

import { useAITaskTask, useLLMConnection } from "~/ai/hooks";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { getPersistableGeneratedTitle } from "~/store/zustand/ai-task/task-configs/title-success";
import { isLocalModelProviderId } from "~/store/zustand/ai-task/tasks";

function SummaryTitleSpace({ title }: { title: string }) {
  return (
    <div
      data-testid="summary-title-space"
      className="pointer-events-none mb-4 flex min-h-[1.875rem] items-start"
    >
      {title ? (
        <h1 className="text-foreground text-[1.5rem] leading-[1.875rem] font-bold">
          {title}
        </h1>
      ) : (
        <span
          aria-hidden="true"
          className="text-muted-foreground animate-pulse text-[1.5rem] leading-[1.875rem] font-bold opacity-60"
        >
          <Trans>Generating title...</Trans>
        </span>
      )}
    </div>
  );
}

export function StreamingView({
  sessionId,
  sessionTitle,
  enhancedNoteId,
}: {
  sessionId: string;
  sessionTitle: string;
  enhancedNoteId: string;
}) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const { streamedText, isGenerating, currentStep } = useAITaskTask(
    taskId,
    "enhance",
  );
  const { conn } = useLLMConnection();
  const isLocalModel = !!conn && isLocalModelProviderId(conn.providerId);
  const isReasoning = currentStep?.type === "reasoning";
  const titleTaskId = createTaskId(sessionId, "title");
  const { streamedText: streamedTitle, isGenerating: isGeneratingTitle } =
    useAITaskTask(titleTaskId, "title");
  const title = sessionTitle.trim();
  const generatedTitle = isGeneratingTitle
    ? ""
    : getPersistableGeneratedTitle(streamedTitle);
  const visibleTitle = title || generatedTitle;

  if (streamedText.trim().length === 0) {
    return (
      <div
        role="status"
        aria-live="polite"
        className="text-muted-foreground flex flex-col gap-0.5 pb-2 text-sm"
      >
        <p className="animate-pulse leading-5">
          {isReasoning ? (
            <Trans>Model is thinking...</Trans>
          ) : (
            <Trans>Analyzing structure...</Trans>
          )}
        </p>
        <p className="flex items-start gap-1.5 pl-4 text-xs leading-5">
          <span
            aria-hidden="true"
            className="border-muted-foreground/60 mt-[5px] h-2 w-2 shrink-0 rounded-bl-[2px] border-b border-l"
          />
          <span>
            {isReasoning ? (
              <Trans>
                Reasoning models think through the transcript before writing.
              </Trans>
            ) : isLocalModel ? (
              <Trans>
                On-device models can take a few minutes to warm up before text
                appears.
              </Trans>
            ) : (
              <Trans>Tip: The Anarlog team loves our users!</Trans>
            )}
          </span>
        </p>
      </div>
    );
  }

  return (
    <div className="pb-2">
      <div className="flex flex-col gap-1">
        <SummaryTitleSpace title={visibleTitle} />
        <Streamdown
          icons={streamdownIcons}
          components={streamdownComponents}
          className={cn(["note-typography", "flex flex-col"])}
          caret="block"
          isAnimating={isGenerating}
        >
          {streamedText}
        </Streamdown>
      </div>
    </div>
  );
}
