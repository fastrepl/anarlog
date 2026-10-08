import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { useEnhancedNoteRecords } from "~/session/queries";

export function useHasEditableSummary(sessionId: string | undefined) {
  const notes = useEnhancedNoteRecords(sessionId ?? "");
  const isEnhancing = useIsSessionEnhancing(sessionId ?? "");
  return !isEnhancing && notes.some((note) => hasSummaryText(note.content));
}

export function hasSummaryText(content: string): boolean {
  const trimmed = content.trim();
  if (!trimmed) {
    return false;
  }
  try {
    return hasTextNode(JSON.parse(trimmed));
  } catch {
    return true;
  }
}

function hasTextNode(node: unknown): boolean {
  if (typeof node !== "object" || node === null) {
    return false;
  }
  const { text, content } = node as { text?: unknown; content?: unknown };
  if (typeof text === "string" && text.trim()) {
    return true;
  }
  return Array.isArray(content) && content.some(hasTextNode);
}
