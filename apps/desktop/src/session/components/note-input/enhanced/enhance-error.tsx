import { Trans } from "@lingui/react/macro";
import { ArrowsClockwise, WarningCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

import { useAITask } from "~/ai/contexts";
import { useLanguageModel } from "~/ai/hooks";
import { useAuth } from "~/auth";
import { useEnhancedNote } from "~/session/queries";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";

export function EnhanceError({
  sessionId,
  enhancedNoteId,
  error,
  isUnauthenticated,
}: {
  sessionId: string;
  enhancedNoteId: string;
  error: Error | undefined;
  isUnauthenticated: boolean;
}) {
  const auth = useAuth();
  const model = useLanguageModel("enhance");
  const generate = useAITask((state) => state.generate);
  const templateId = useEnhancedNote(enhancedNoteId)?.templateId || undefined;
  const signInMutation = useMutation({ mutationFn: () => auth.signIn() });

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
    <div role="alert" {...stylex.props(styles.root)}>
      <WarningCircle aria-hidden {...stylex.props(styles.icon)} />
      <div {...stylex.props(styles.copy)}>
        <p {...stylex.props(styles.title)}>
          {isUnauthenticated ? (
            <Trans>Sign in to generate this summary</Trans>
          ) : (
            <Trans>Summary generation failed</Trans>
          )}
        </p>
        <p {...stylex.props(styles.description)}>
          {isUnauthenticated ? (
            <Trans>
              Anarlog could not generate this summary because you were not
              signed in. Sign in, then try again.
            </Trans>
          ) : (
            error?.message || (
              <Trans>Something went wrong while generating the summary.</Trans>
            )
          )}
        </p>
      </div>
      {isUnauthenticated ? (
        <Button
          onClick={() => signInMutation.mutate()}
          disabled={signInMutation.isPending}
          size="sm"
          variant="default"
        >
          {signInMutation.isPending ? (
            <Trans>Opening…</Trans>
          ) : (
            <Trans>Sign in</Trans>
          )}
        </Button>
      ) : (
        <Button
          onClick={handleRetry}
          disabled={!model}
          size="sm"
          sx={styles.button}
          variant="default"
        >
          <ArrowsClockwise size={16} />
          <span>
            <Trans>Retry</Trans>
          </span>
        </Button>
      )}
    </div>
  );
}

const styles = stylex.create({
  button: {
    gap: "0.5rem",
  },
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    marginBottom: "1.5rem",
    maxWidth: "28rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
  },
  icon: {
    color: colors.mutedForeground,
    height: "2.25rem",
    marginBottom: "1.25rem",
    strokeWidth: 1.5,
    width: "2.25rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    height: "100%",
    justifyContent: "center",
    minHeight: "400px",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  title: {
    fontSize: "1rem",
    fontWeight: 500,
  },
});
