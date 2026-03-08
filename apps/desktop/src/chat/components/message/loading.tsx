import { Loader2 } from "lucide-react";

import { MessageBubble, MessageContainer } from "./shared";

export function LoadingMessage() {
  return (
    <MessageContainer align="start">
      <MessageBubble variant="loading">
        <div className="flex items-center gap-2">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span className="text-sm">Thinking...</span>
        </div>
      </MessageBubble>
    </MessageContainer>
  );
}
