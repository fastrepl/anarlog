import type { ContextRef } from "./entities";

const SESSION_CONTEXT_DRAG_TYPE = "application/x-anarlog-session-context";

type SessionDragPayload = {
  sessionId: string;
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
  dataTransfer.effectAllowed = "copy";
  dataTransfer.setData(
    SESSION_CONTEXT_DRAG_TYPE,
    JSON.stringify({ sessionId }),
  );
  dataTransfer.setData("text/plain", fallbackText);
};

export const readSessionContextDragData = (
  dataTransfer: Pick<DataTransfer, "getData" | "types"> | null | undefined,
): ContextRef | null => {
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

    return createSessionContextRef(payload.sessionId);
  } catch {
    return null;
  }
};
