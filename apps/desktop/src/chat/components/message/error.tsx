import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowCounterClockwise, ArrowSquareOut } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";

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
  const { t } = useLingui();
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
        <p {...stylex.props(styles.message)}>{error.message}</p>
        {showContextLengthHelp && (
          <button onClick={handleOpenFaq} {...stylex.props(styles.helpButton)}>
            <ArrowSquareOut {...stylex.props(styles.icon)} />
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

const styles = stylex.create({
  message: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  helpButton: {
    alignItems: "center",
    color: {
      default: colors.alertForeground,
      ":hover": "oklch(39.6% 0.141 25.723)",
    },
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    marginTop: "0.5rem",
    textDecorationLine: "underline",
  },
  icon: {
    height: "0.75rem",
    width: "0.75rem",
  },
});
