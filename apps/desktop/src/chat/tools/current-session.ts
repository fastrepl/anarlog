import type { ToolDependencies } from "./types";

export type ChatToolContext = {
  currentSessionId: string | undefined;
};

function isChatToolContext(value: unknown): value is ChatToolContext {
  return (
    typeof value === "object" &&
    value !== null &&
    "currentSessionId" in value &&
    (value.currentSessionId === undefined ||
      typeof value.currentSessionId === "string")
  );
}

export type ToolCallOptions = { experimental_context?: unknown };

// Defaults to the note attached to the triggering user message.
export function resolveCurrentSessionId(
  deps: Pick<ToolDependencies, "getSessionId">,
  options: ToolCallOptions | undefined,
): string | undefined {
  const context = options?.experimental_context;
  if (isChatToolContext(context)) {
    return context.currentSessionId;
  }
  return deps.getSessionId();
}

export function resolveActiveEnhancedNoteId(
  deps: Pick<ToolDependencies, "getSessionId" | "getEnhancedNoteId">,
  sessionId: string,
): string | undefined {
  return deps.getSessionId() === sessionId
    ? deps.getEnhancedNoteId()
    : undefined;
}
