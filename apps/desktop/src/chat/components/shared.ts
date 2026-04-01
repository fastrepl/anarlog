import type { HyprUIMessage } from "~/chat/types";

export function hasRenderableContent(message: HyprUIMessage): boolean {
  return message.parts.some((part) => {
    if (part.type === "step-start") {
      return false;
    }

    if (part.type === "reasoning") {
      return part.text.trim().length > 0;
    }

    return true;
  });
}
