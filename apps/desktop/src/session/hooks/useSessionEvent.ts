import { useMemo } from "react";

import { useSessionCalendarEvent } from "~/calendar/queries";
import { useSession } from "~/session/queries";
import { getSessionEvent } from "~/session/utils";

export function useSessionEvent(sessionId: string) {
  const session = useSession(sessionId);
  const calendarEvent = useSessionCalendarEvent(sessionId);
  return useMemo(
    () => calendarEvent ?? (session ? getSessionEvent(session) : null),
    [calendarEvent, session],
  );
}
