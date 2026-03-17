import type { HyprUIMessage } from "../types";
import { CONTEXT_ENTITY_SOURCES } from "./entities";
import type { ContextRef } from "./entities";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

const validSources: ReadonlySet<string> = new Set(CONTEXT_ENTITY_SOURCES);

export function isContextRef(value: unknown): value is ContextRef {
  return (
    isRecord(value) &&
    value.kind === "session" &&
    typeof value.key === "string" &&
    typeof value.sessionId === "string" &&
    (value.source === undefined ||
      (typeof value.source === "string" && validSources.has(value.source)))
  );
}

export function getContextRefs(metadata: unknown): ContextRef[] {
  if (!isRecord(metadata) || !Array.isArray(metadata.contextRefs)) {
    return [];
  }

  return metadata.contextRefs.filter((ref): ref is ContextRef =>
    isContextRef(ref),
  );
}

export function extractContextRefsFromMessages(
  messages: Array<Pick<HyprUIMessage, "role" | "metadata">>,
): ContextRef[] {
  const seen = new Set<string>();
  const refs: ContextRef[] = [];

  for (const msg of messages) {
    if (msg.role !== "user") continue;
    for (const ref of getContextRefs(msg.metadata)) {
      if (!seen.has(ref.key)) {
        seen.add(ref.key);
        refs.push(ref);
      }
    }
  }

  return refs;
}
