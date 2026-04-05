import { useEffect, useState } from "react";

import { format } from "@hypr/utils";

import { useTimezone, toTz } from "~/calendar/hooks";

export function useToday() {
  const tz = useTimezone();
  const [today, setToday] = useState(() =>
    format(toTz(new Date(), tz), "yyyy-MM-dd"),
  );

  useEffect(() => {
    const interval = setInterval(() => {
      setToday(format(toTz(new Date(), tz), "yyyy-MM-dd"));
    }, 60000);
    return () => clearInterval(interval);
  }, [tz]);

  return today;
}
