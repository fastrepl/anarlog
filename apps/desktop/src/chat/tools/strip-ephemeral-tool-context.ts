import { CONTEXT_TEXT_FIELD } from "./index";

import type { HyprUIMessage } from "~/chat/types";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function stripEphemeralToolContext(
  parts: HyprUIMessage["parts"],
): HyprUIMessage["parts"] {
  let changed = false;
  const sanitized = parts.map((part) => {
    if (
      !isRecord(part) ||
      part.type !== "tool-search_sessions" ||
      part.state !== "output-available" ||
      !isRecord(part.output) ||
      !(CONTEXT_TEXT_FIELD in part.output)
    ) {
      return part;
    }

    changed = true;
    const { contextText: _contextText, ...restOutput } = part.output;
    return {
      ...part,
      output: restOutput,
    };
  });

  return changed ? sanitized : parts;
}
