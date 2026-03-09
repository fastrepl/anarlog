import { useEffect, useState } from "react";

import { useSessionEvent } from "~/store/tinybase/hooks";

export function useEventCountdown(sessionId: string): string | null {
  const sessionEvent = useSessionEvent(sessionId);
  const startedAt = sessionEvent?.started_at;

  const [countdown, setCountdown] = useState<string | null>(null);

  useEffect(() => {
    if (!startedAt) {
      setCountdown(null);
      return;
    }

    const eventStart = new Date(startedAt).getTime();

    const updateCountdown = () => {
      const now = Date.now();
      const diff = eventStart - now;
      const fiveMinutes = 5 * 60 * 1000;

      if (diff <= 0 || diff > fiveMinutes) {
        setCountdown(null);
        return;
      }

      const totalSeconds = Math.floor(diff / 1000);
      const mins = Math.floor(totalSeconds / 60);
      const secs = totalSeconds % 60;
      if (mins > 0) {
        setCountdown(`meeting starts in ${mins} mins ${secs} seconds`);
      } else {
        setCountdown(`meeting starts in ${secs} seconds`);
      }
    };

    updateCountdown();
    const interval = setInterval(updateCountdown, 1000);
    return () => clearInterval(interval);
  }, [startedAt]);

  return countdown;
}
