import type { GoogleCalendarDataState } from "../data-boundary";
import {
  GOOGLE_CALENDAR_DATA_SQL,
  mapGoogleCalendarDataRows,
  type GoogleCalendarDataSqlRow,
} from "../google-calendar-data";

import { useLiveQuery } from "~/db";

export function getGoogleCalendarDataState({
  data,
  isLoading,
  error,
}: {
  data: boolean | undefined;
  isLoading: boolean;
  error: Error | null;
}): GoogleCalendarDataState {
  if (error) {
    return "error";
  }
  if (isLoading || data === undefined) {
    return "loading";
  }
  return data ? "present" : "absent";
}

export function useGoogleCalendarDataState(): GoogleCalendarDataState {
  const query = useLiveQuery<GoogleCalendarDataSqlRow, boolean>({
    sql: GOOGLE_CALENDAR_DATA_SQL,
    mapRows: mapGoogleCalendarDataRows,
  });

  return getGoogleCalendarDataState(query);
}
