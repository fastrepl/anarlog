import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { Streamdown } from "streamdown";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { streamdownComponents } from "../../streamdown";

import { useAITaskTask, useLLMConnection } from "~/ai/hooks";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { getPersistableGeneratedTitle } from "~/store/zustand/ai-task/task-configs/title-success";
import { isLocalModelProviderId } from "~/store/zustand/ai-task/tasks";

function SummaryTitleSpace({ title }: { title: string }) {
  return (
    <div data-testid="summary-title-space" {...stylex.props(styles.titleSpace)}>
      {title ? (
        <h1 {...stylex.props(styles.title)}>{title}</h1>
      ) : (
        <span
          aria-hidden="true"
          {...stylex.props(styles.title, styles.placeholder)}
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
      <div role="status" aria-live="polite" {...stylex.props(styles.status)}>
        <p {...stylex.props(styles.pulse, styles.statusLine)}>
          {isReasoning ? (
            <Trans>Model is thinking...</Trans>
          ) : (
            <Trans>Analyzing structure...</Trans>
          )}
        </p>
        <p {...stylex.props(styles.tip)}>
          <span aria-hidden="true" {...stylex.props(styles.tipBranch)} />
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
    <div {...stylex.props(styles.bottomPadding)}>
      <div {...stylex.props(styles.result)}>
        <SummaryTitleSpace title={visibleTitle} />
        <Streamdown
          components={streamdownComponents}
          {...mergeStyleXProps(styles.stream, "note-typography")}
          controls={false}
          isAnimating={isGenerating}
        >
          {streamedText}
        </Streamdown>
      </div>
    </div>
  );
}

const pulse = stylex.keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.5 },
});

const styles = stylex.create({
  bottomPadding: {
    paddingBottom: "0.5rem",
  },
  placeholder: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    color: colors.mutedForeground,
    opacity: 0.6,
  },
  pulse: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: pulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
  },
  result: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
  },
  status: {
    color: colors.mutedForeground,
    display: "flex",
    flexDirection: "column",
    fontSize: "0.875rem",
    gap: "0.125rem",
    paddingBottom: "0.5rem",
  },
  statusLine: {
    lineHeight: "1.25rem",
  },
  stream: {
    display: "flex",
    flexDirection: "column",
  },
  tip: {
    alignItems: "flex-start",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.375rem",
    lineHeight: "1.25rem",
    paddingLeft: "1rem",
  },
  tipBranch: {
    borderBottomColor: `color-mix(in oklab, ${colors.mutedForeground} 60%, transparent)`,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderLeftColor: `color-mix(in oklab, ${colors.mutedForeground} 60%, transparent)`,
    borderLeftStyle: "solid",
    borderLeftWidth: "1px",
    borderBottomLeftRadius: "2px",
    flexShrink: 0,
    height: "0.5rem",
    marginTop: "5px",
    width: "0.5rem",
  },
  title: {
    color: colors.foreground,
    fontSize: "1.5rem",
    fontWeight: 700,
    lineHeight: "1.875rem",
  },
  titleSpace: {
    alignItems: "flex-start",
    display: "flex",
    marginBottom: "1rem",
    minHeight: "1.875rem",
    pointerEvents: "none",
  },
});
