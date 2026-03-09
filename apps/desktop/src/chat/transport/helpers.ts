import { CONTEXT_TEXT_FIELD } from "../tools";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export const MAX_TOOL_STEPS = 5;
export const MESSAGE_WINDOW_THRESHOLD = 20;
export const MESSAGE_WINDOW_SIZE = 10;

export function getSessionIdsFromSearchOutput(output: unknown): string[] {
  if (!isRecord(output) || !Array.isArray(output.results)) {
    return [];
  }
  return output.results.flatMap((item) => {
    if (
      !isRecord(item) ||
      (typeof item.id !== "string" && typeof item.id !== "number")
    ) {
      return [];
    }
    return [String(item.id)];
  });
}

export type ToolOutputPart = {
  type: `tool-${string}`;
  state: "output-available";
  output?: unknown;
  [key: string]: unknown;
};

export function isToolOutputPart(value: unknown): value is ToolOutputPart {
  return (
    isRecord(value) &&
    typeof value.type === "string" &&
    value.type.startsWith("tool-") &&
    value.state === "output-available"
  );
}

export function hasContextText(output: unknown): boolean {
  if (!isRecord(output)) return false;
  const contextText = output[CONTEXT_TEXT_FIELD];
  return typeof contextText === "string" && contextText.length > 0;
}
