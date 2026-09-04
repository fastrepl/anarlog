import { Trans, useLingui } from "@lingui/react/macro";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import {
  ArrowCounterClockwise,
  ArrowSquareOut,
} from "@anlg/ui/components/icons";

import { ActionButton, MessageBubble, MessageContainer } from "./shared";

import { env } from "~/env";

const WEB_APP_BASE_URL = env.VITE_APP_URL ?? "http://localhost:3000";

// `useChat().error` is typed as Error but holds whatever the transport threw.
export function getChatErrorText(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return typeof error === "string" ? error : String(error);
}

function isContextLengthError(message: string): boolean {
  const lowerMessage = message.toLowerCase();
  return (
    (lowerMessage.includes("n_keep") && lowerMessage.includes("n_ctx")) ||
    (lowerMessage.includes("context") && lowerMessage.includes("exceeds")) ||
    lowerMessage.includes("context length") ||
    lowerMessage.includes("context size")
  );
}

export function ErrorMessage({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry?: () => void;
}) {
  const { t } = useLingui();
  const message = getChatErrorText(error);
  const showContextLengthHelp = isContextLengthError(message);

  const handleOpenFaq = () => {
    void openerCommands.openUrl(
      `${WEB_APP_BASE_URL}/docs/faq/local-llm-setup#context-length-error`,
      null,
    );
  };

  return (
    <MessageContainer align="start">
      <MessageBubble variant="error" withActionButton={!!onRetry}>
        <p className="text-sm">{message}</p>
        {showContextLengthHelp && (
          <button
            onClick={handleOpenFaq}
            className="mt-2 flex items-center gap-1 text-xs text-red-700 underline hover:text-red-900"
          >
            <ArrowSquareOut className="h-3 w-3" />
            <Trans>Learn how to fix this</Trans>
          </button>
        )}
        {onRetry && (
          <ActionButton
            onClick={onRetry}
            variant="error"
            icon={ArrowCounterClockwise}
            label={t`Retry`}
          />
        )}
      </MessageBubble>
    </MessageContainer>
  );
}
