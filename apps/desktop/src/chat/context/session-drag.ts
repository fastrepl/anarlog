import type { ContextRef } from "./entities";

const SESSION_CONTEXT_DRAG_TYPE = "application/x-anarlog-session-context";

type SessionDragPayload = {
  sessionId: string;
  title?: string;
};

type SessionMentionDragData = {
  id: string;
  label: string;
};

const createSessionContextRef = (sessionId: string): ContextRef => ({
  kind: "session",
  key: `session:manual:${sessionId}`,
  source: "manual",
  sessionId,
});

export const hasSessionContextDragData = (
  dataTransfer: Pick<DataTransfer, "types"> | null | undefined,
) => {
  if (!dataTransfer) {
    return false;
  }

  return Array.from(dataTransfer.types).includes(SESSION_CONTEXT_DRAG_TYPE);
};

export const writeSessionContextDragData = (
  dataTransfer: DataTransfer,
  sessionId: string,
  fallbackText: string,
) => {
  const title = fallbackText.trim() || "Untitled";

  dataTransfer.effectAllowed = "copy";
  dataTransfer.setData(
    SESSION_CONTEXT_DRAG_TYPE,
    JSON.stringify({ sessionId, title }),
  );
  dataTransfer.setData("text/plain", title);
};

const readSessionContextDragPayload = (
  dataTransfer: Pick<DataTransfer, "getData" | "types"> | null | undefined,
): SessionDragPayload | null => {
  if (!dataTransfer || !hasSessionContextDragData(dataTransfer)) {
    return null;
  }

  try {
    const payload = JSON.parse(
      dataTransfer.getData(SESSION_CONTEXT_DRAG_TYPE),
    ) as SessionDragPayload;

    if (
      typeof payload.sessionId !== "string" ||
      payload.sessionId.trim().length === 0
    ) {
      return null;
    }

    return {
      sessionId: payload.sessionId,
      title:
        typeof payload.title === "string" && payload.title.trim().length > 0
          ? payload.title.trim()
          : undefined,
    };
  } catch {
    return null;
  }
};

export const readSessionContextDragData = (
  dataTransfer: Pick<DataTransfer, "getData" | "types"> | null | undefined,
): ContextRef | null => {
  const payload = readSessionContextDragPayload(dataTransfer);
  return payload ? createSessionContextRef(payload.sessionId) : null;
};

export const readSessionMentionDragData = (
  dataTransfer: Pick<DataTransfer, "getData" | "types"> | null | undefined,
): SessionMentionDragData | null => {
  const payload = readSessionContextDragPayload(dataTransfer);
  if (!payload) {
    return null;
  }

  return {
    id: payload.sessionId,
    label: payload.title ?? "Untitled",
  };
};
