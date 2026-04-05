import type { JSONContent } from "./index";

export function getNodeTextContent(node: JSONContent): string {
  if (typeof node.text === "string") {
    return node.text;
  }

  return (node.content ?? []).map(getNodeTextContent).join("");
}

function buildTextContent(text: string): JSONContent[] | undefined {
  return text ? [{ type: "text", text }] : undefined;
}

function buildSessionNode(
  sessionId: string,
  title: string,
  checked?: boolean,
): JSONContent {
  return {
    type: "session",
    attrs:
      typeof checked === "boolean" ? { sessionId, checked } : { sessionId },
    content: buildTextContent(title),
  };
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
  const linkedSessionNodes: JSONContent[] = [];

  const pushSessionNode = (
    sessionId: string,
    preferredTitle?: string,
    preferredChecked?: boolean,
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
    linkedSessionNodes.push(
      buildSessionNode(
        normalizedSessionId,
        preferredTitle ?? getSessionTitle(normalizedSessionId),
        preferredChecked,
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
        typeof node.attrs?.checked === "boolean"
          ? node.attrs.checked
          : undefined,
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

  const userContent = existingContent.filter(
    (node) => node.type !== "event" && node.type !== "session",
  );
  const merged = [...linkedSessionNodes, ...userContent];

  if (merged.length === 0) {
    merged.push({ type: "paragraph" });
  }

  return { type: "doc", content: merged };
}
