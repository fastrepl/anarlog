import type { TaskStatus } from "../tasks";
import { createTaskStatusAttrs, getOptionalTaskStatus } from "../tasks";
import type { JSONContent } from "./index";

export function getNodeTextContent(node: JSONContent): string {
  if (typeof node.text === "string") {
    return node.text;
  }

  return (node.content ?? []).map(getNodeTextContent).join("");
}

function buildSessionTitleContent(text: string): JSONContent[] {
  return [
    {
      type: "paragraph",
      content: text ? [{ type: "text", text }] : undefined,
    },
  ];
}

function buildSessionNodeWithContent(
  sessionId: string,
  content: JSONContent[],
  status?: TaskStatus,
): JSONContent {
  return {
    type: "session",
    attrs: status
      ? { sessionId, ...createTaskStatusAttrs(status) }
      : { sessionId },
    content,
  };
}

function buildSessionNode(
  sessionId: string,
  title: string,
  status?: TaskStatus,
): JSONContent {
  return buildSessionNodeWithContent(
    sessionId,
    buildSessionTitleContent(title),
    status,
  );
}

export function mergeLinkedSessionsIntoContent({
  content,
  eventIds,
  sessionIds,
  resolveEventSessionId,
  getSessionTitle,
  normalizeSessionId,
  keepLinkedSession,
}: {
  content: JSONContent;
  eventIds: string[];
  sessionIds: string[];
  resolveEventSessionId: (eventId: string) => string | null;
  getSessionTitle: (sessionId: string) => string;
  normalizeSessionId?: (sessionId: string) => string;
  keepLinkedSession?: (sessionId: string) => boolean;
}): JSONContent {
  const existingContent =
    content.type === "doc" ? (content.content ?? []) : ([] as JSONContent[]);
  const seenSessionIds = new Set<string>();
  const canonicalSessionIds: string[] = [];
  const sessionNodeById = new Map<string, JSONContent>();

  const pushSessionNode = (
    sessionId: string,
    preferredTitle?: string,
    preferredStatus?: TaskStatus,
    preferredContent?: JSONContent[],
  ) => {
    const normalizedSessionId = normalizeSessionId?.(sessionId) ?? sessionId;
    if (
      !normalizedSessionId ||
      seenSessionIds.has(normalizedSessionId) ||
      (keepLinkedSession && !keepLinkedSession(normalizedSessionId))
    ) {
      return;
    }

    seenSessionIds.add(normalizedSessionId);
    canonicalSessionIds.push(normalizedSessionId);
    sessionNodeById.set(
      normalizedSessionId,
      preferredContent
        ? buildSessionNodeWithContent(
            normalizedSessionId,
            preferredContent,
            preferredStatus,
          )
        : buildSessionNode(
            normalizedSessionId,
            preferredTitle ?? getSessionTitle(normalizedSessionId),
            preferredStatus,
          ),
    );
  };

  for (const node of existingContent) {
    if (node.type === "session") {
      const sessionId = node.attrs?.sessionId;
      if (typeof sessionId !== "string" || sessionId === "") {
        continue;
      }

      pushSessionNode(
        sessionId,
        getNodeTextContent(node) || getSessionTitle(sessionId),
        getOptionalTaskStatus(node.attrs?.status, node.attrs?.checked) ??
          undefined,
        node.content ?? buildSessionTitleContent(getSessionTitle(sessionId)),
      );
      continue;
    }

    if (node.type === "event") {
      const eventId = node.attrs?.eventId;
      if (typeof eventId !== "string" || eventId === "") {
        continue;
      }

      const sessionId = resolveEventSessionId(eventId);
      if (!sessionId) {
        continue;
      }

      pushSessionNode(
        sessionId,
        getNodeTextContent(node) || getSessionTitle(sessionId),
      );
    }
  }

  for (const eventId of eventIds) {
    const sessionId = resolveEventSessionId(eventId);
    if (sessionId) {
      pushSessionNode(sessionId);
    }
  }

  for (const sessionId of sessionIds) {
    pushSessionNode(sessionId);
  }

  const placedSessionIds = new Set<string>();
  const merged: JSONContent[] = [];
  let lastLinkedIndex = -1;

  for (const node of existingContent) {
    if (node.type === "session") {
      const sessionId = node.attrs?.sessionId;
      if (typeof sessionId !== "string" || sessionId === "") {
        continue;
      }

      const normalizedSessionId = normalizeSessionId?.(sessionId) ?? sessionId;
      if (
        !normalizedSessionId ||
        placedSessionIds.has(normalizedSessionId) ||
        !sessionNodeById.has(normalizedSessionId)
      ) {
        continue;
      }

      merged.push(sessionNodeById.get(normalizedSessionId)!);
      placedSessionIds.add(normalizedSessionId);
      lastLinkedIndex = merged.length - 1;
      continue;
    }

    if (node.type === "event") {
      const eventId = node.attrs?.eventId;
      if (typeof eventId !== "string" || eventId === "") {
        continue;
      }

      const sessionId = resolveEventSessionId(eventId);
      const normalizedSessionId = sessionId
        ? (normalizeSessionId?.(sessionId) ?? sessionId)
        : null;
      if (
        !normalizedSessionId ||
        placedSessionIds.has(normalizedSessionId) ||
        !sessionNodeById.has(normalizedSessionId)
      ) {
        continue;
      }

      merged.push(sessionNodeById.get(normalizedSessionId)!);
      placedSessionIds.add(normalizedSessionId);
      lastLinkedIndex = merged.length - 1;
      continue;
    }

    merged.push(node);
  }

  const missingLinkedNodes = canonicalSessionIds
    .filter((sessionId) => !placedSessionIds.has(sessionId))
    .map((sessionId) => sessionNodeById.get(sessionId)!);

  if (missingLinkedNodes.length > 0) {
    if (lastLinkedIndex >= 0) {
      merged.splice(lastLinkedIndex + 1, 0, ...missingLinkedNodes);
    } else {
      merged.unshift(...missingLinkedNodes);
    }
  }

  if (merged.length === 0) {
    merged.push({ type: "paragraph" });
  }

  return { type: "doc", content: merged };
}
