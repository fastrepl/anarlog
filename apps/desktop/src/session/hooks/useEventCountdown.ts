import { useEffect, useRef, useState } from "react";

import { useSessionEvent } from "~/session/hooks/useSessionEvent";

const FIVE_MINUTES = 5 * 60 * 1000;

export function useEventCountdown(
  sessionId: string,
  { onExpire }: { onExpire?: () => void } = {},
) {
  const sessionEvent = useSessionEvent(sessionId);
  const startedAt = sessionEvent?.started_at;
  const onExpireRef = useRef(onExpire);
  onExpireRef.current = onExpire;

  const [label, setLabel] = useState<string | null>(null);

  useEffect(() => {
    if (!startedAt) {
      setLabel(null);
      return;
    }

    const eventStart = new Date(startedAt).getTime();
    let fired = false;
    let armed = false;

    let interval: ReturnType<typeof setInterval>;

    const update = () => {
      const diff = eventStart - Date.now();

      if (diff <= 0) {
        setLabel(null);
        clearInterval(interval);
        if (armed && !fired) {
          fired = true;
          onExpireRef.current?.();
        }
        return;
      }

      if (diff > FIVE_MINUTES) {
        setLabel(null);
        return;
      }

      armed = true;
      const totalSeconds = Math.floor(diff / 1000);
      const mins = Math.floor(totalSeconds / 60);
      const secs = totalSeconds % 60;
      setLabel(mins > 0 ? `starts in ${mins}m ${secs}s` : `starts in ${secs}s`);
    };

    update();
    interval = setInterval(update, 1000);
    return () => clearInterval(interval);
  }, [startedAt]);

  return { label };
}
