import type { HyprUIMessage } from "~/chat/types";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function hasRenderableParts(parts: unknown): boolean {
  if (!Array.isArray(parts)) {
    return false;
  }

  return parts.some((part) => {
    if (!isRecord(part) || typeof part.type !== "string") {
      return false;
    }

    if (part.type === "step-start") {
      return false;
    }

    if (part.type === "reasoning" || part.type === "text") {
      return typeof part.text === "string" && part.text.trim().length > 0;
    }

    return true;
  });
}

export function hasRenderableContent(message: HyprUIMessage) {
  return hasRenderableParts(message.parts);
}
