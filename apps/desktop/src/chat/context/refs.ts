import type { HyprUIMessage } from "../types";
import { CONTEXT_ENTITY_SOURCES } from "./entities";
import type { ContextRef } from "./entities";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

const validSources: ReadonlySet<string> = new Set(CONTEXT_ENTITY_SOURCES);

export function isContextRef(value: unknown): value is ContextRef {
  if (!isRecord(value) || typeof value.key !== "string") {
    return false;
  }

  if (
    value.source !== undefined &&
    (typeof value.source !== "string" || !validSources.has(value.source))
  ) {
    return false;
  }

  if (value.kind === "session") {
    return typeof value.sessionId === "string";
  }

  if (value.kind === "human") {
    return typeof value.humanId === "string";
  }

  if (value.kind === "organization") {
    return typeof value.organizationId === "string";
  }

  return false;
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
