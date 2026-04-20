import { useEffect, useState } from "react";

import { format } from "@hypr/utils";

function formatDate(date: Date) {
  return format(date, "yyyy-MM-dd");
}

export function useToday() {
  const [today, setToday] = useState(() => formatDate(new Date()));

  useEffect(() => {
    const interval = window.setInterval(() => {
      setToday(formatDate(new Date()));
    }, 60_000);

    return () => window.clearInterval(interval);
  }, []);

  return today;
}
