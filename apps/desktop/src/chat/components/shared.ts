import type { HyprUIMessage } from "~/chat/types";

export function hasRenderableContent(message: HyprUIMessage): boolean {
  return message.parts.some((part) => part.type !== "step-start");
}
