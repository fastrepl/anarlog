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

// Mirrors the summary `edit_summary` rewrites when no note id is requested.
export function resolveSummaryTargetId(
  notes: ReadonlyArray<{ id: string; templateId?: string | null }>,
  activeEnhancedNoteId: string | undefined,
): string | null {
  if (
    activeEnhancedNoteId &&
    notes.some((note) => note.id === activeEnhancedNoteId)
  ) {
    return activeEnhancedNoteId;
  }
  return (
    notes.find((note) => !note.templateId)?.id ??
    (notes.length === 1 ? notes[0].id : null)
  );
}
