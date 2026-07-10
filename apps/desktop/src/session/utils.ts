import type { SessionEvent } from "@hypr/store";

import type * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

export function getSessionEvent(session: {
  event_json?: string | null;
}): SessionEvent | null {
  const eventJson = session.event_json;
  if (!eventJson) return null;
  try {
    return JSON.parse(eventJson) as SessionEvent;
  } catch {
    return null;
  }
}

export function getSessionEventById(
  store: Store,
  sessionId: string,
): SessionEvent | null {
  const row = store.getRow("sessions", sessionId);
  if (!row) return null;
  return getSessionEvent(row);
}
