import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";

import { ArrowsClockwise, WarningCircle } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { useAITask } from "~/ai/contexts";
import { useLanguageModel } from "~/ai/hooks";
import { useAuth } from "~/auth";
import { useEnhancedNote } from "~/session/queries";
import {
  type AIErrorSource,
  getAIErrorSource,
} from "~/store/zustand/ai-task/shared/error-source";
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
    <div
      role="alert"
      className="flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center"
    >
      <WarningCircle
        aria-hidden
        className="text-muted-foreground mb-5 size-9 stroke-[1.5]"
      />
      <div className="mb-6 flex max-w-md flex-col gap-2">
        <p className="text-base font-medium">
          {isUnauthenticated ? (
            <Trans>Sign in to generate this summary</Trans>
          ) : (
            <Trans>Summary generation failed</Trans>
          )}
        </p>
        <p className="text-muted-foreground text-sm leading-relaxed">
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
        {!isUnauthenticated && <ErrorSourceNote error={error} />}
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
          className="gap-2"
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

function ErrorSourceNote({ error }: { error: Error | undefined }) {
  const { t } = useLingui();
  const source = getAIErrorSource(error);
  if (!source) {
    return null;
  }

  return (
    <p className="text-muted-foreground text-xs">
      {formatErrorSource(source, t)}
    </p>
  );
}

function formatErrorSource(
  source: AIErrorSource,
  t: ReturnType<typeof useLingui>["t"],
): string {
  switch (source.kind) {
    case "provider": {
      const provider = source.provider;
      return provider
        ? t`Source: ${provider} (AI model provider)`
        : t`Source: AI model provider`;
    }
    case "openrouter":
      return t`Source: OpenRouter (AI routing service)`;
    case "upstream":
      return t`Source: OpenRouter or the AI model provider`;
    case "anarlog":
      return t`Source: Anarlog`;
  }
}
