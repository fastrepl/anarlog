import { Trans } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";

import { MessageBubble, MessageContainer } from "./shared";

export function LoadingMessage() {
  return (
    <MessageContainer align="start">
      <MessageBubble variant="loading">
        <div className="flex items-center gap-2">
          <CircleNotch className="h-4 w-4 animate-spin" />
          <span className="text-sm">
            <Trans>Thinking...</Trans>
          </span>
        </div>
      </MessageBubble>
    </MessageContainer>
  );
}
