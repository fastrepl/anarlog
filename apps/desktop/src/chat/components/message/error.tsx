import { ExternalLink, RotateCcw } from "lucide-react";

import { commands as openerCommands } from "@hypr/plugin-opener2";

import { ActionButton, MessageBubble, MessageContainer } from "./shared";

import { env } from "~/env";

const WEB_APP_BASE_URL = env.VITE_APP_URL ?? "http://localhost:3000";

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
  error: Error;
  onRetry?: () => void;
}) {
  const showContextLengthHelp = isContextLengthError(error.message);

  const handleOpenFaq = () => {
    void openerCommands.openUrl(
      `${WEB_APP_BASE_URL}/docs/faq/local-llm-setup#context-length-error`,
      null,
    );
  };

  return (
    <MessageContainer align="start">
      <MessageBubble variant="error" withActionButton={!!onRetry}>
        <p className="text-sm">{error.message}</p>
        {showContextLengthHelp && (
          <button
            onClick={handleOpenFaq}
            className="mt-2 flex items-center gap-1 text-xs text-red-700 underline hover:text-red-900"
          >
            <ExternalLink className="h-3 w-3" />
            Learn how to fix this
          </button>
        )}
        {onRetry && (
          <ActionButton
            onClick={onRetry}
            variant="error"
            icon={RotateCcw}
            label="Retry"
          />
        )}
      </MessageBubble>
    </MessageContainer>
  );
}
