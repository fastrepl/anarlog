import type { AnlgUIMessage } from "../types";
import { CONTEXT_ENTITY_SOURCES } from "./entities";
import type { ContextRef } from "./entities";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

const validSources: ReadonlySet<string> = new Set(CONTEXT_ENTITY_SOURCES);

function isContextRef(value: unknown): value is ContextRef {
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

  if (value.kind === "folder") {
    return typeof value.folderId === "string";
  }

  return false;
}

function getContextRefs(metadata: unknown): ContextRef[] {
  if (!isRecord(metadata) || !Array.isArray(metadata.contextRefs)) {
    return [];
  }

  return metadata.contextRefs.filter((ref): ref is ContextRef =>
    isContextRef(ref),
  );
}

export function extractContextRefsFromMessages(
  messages: Array<Pick<AnlgUIMessage, "role" | "metadata">>,
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

function findLastUserMessage<T extends Pick<AnlgUIMessage, "role">>(
  messages: T[],
): T | undefined {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i]?.role === "user") {
      return messages[i];
    }
  }
  return undefined;
}

export function getCurrentSessionIdFromMessages(
  messages: Array<Pick<AnlgUIMessage, "role" | "metadata">>,
): string | undefined {
  const lastUserMessage = findLastUserMessage(messages);
  if (!lastUserMessage) return undefined;

  const ref = getContextRefs(lastUserMessage.metadata).find(
    (candidate) =>
      candidate.kind === "session" && candidate.source === "auto-current",
  );
  return ref?.kind === "session" ? ref.sessionId : undefined;
}

// One entry per session, with the current session last.
export function orderPromptContextRefs(
  refs: ContextRef[],
  currentSessionId: string | undefined,
): ContextRef[] {
  const seenSessionIds = new Set<string>();
  const ordered: ContextRef[] = [];
  let currentRef: ContextRef | undefined;

  for (const ref of refs) {
    if (ref.kind !== "session") {
      ordered.push(ref);
      continue;
    }
    if (ref.sessionId === currentSessionId) {
      currentRef ??= ref;
      continue;
    }
    if (seenSessionIds.has(ref.sessionId)) continue;
    seenSessionIds.add(ref.sessionId);
    ordered.push(ref);
  }

  if (currentRef) ordered.push(currentRef);
  return ordered;
}
