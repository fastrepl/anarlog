import { useMemo } from "react";

import { useSession } from "~/session/queries";
import { getSessionEvent } from "~/session/utils";

export function useSessionEvent(sessionId: string) {
  const session = useSession(sessionId);
  return useMemo(() => (session ? getSessionEvent(session) : null), [session]);
}
