import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo } from "react";

import { googleListCalendars } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { useSync } from "../context";

import { useAuth } from "~/auth";
import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import type { CalendarProvider } from "~/calendar/components/shared";
import { findCalendarByTrackingId } from "~/calendar/utils";
import { env } from "~/env";
import * as main from "~/store/tinybase/store/main";

export function OAuthCalendarSelection({
  groups,
  onToggle,
  isLoading,
}: {
  groups: CalendarGroup[];
  onToggle: (calendar: CalendarItem, enabled: boolean) => void;
  isLoading: boolean;
}) {
  return (
    <CalendarSelection
      groups={groups}
      onToggle={onToggle}
      isLoading={isLoading}
    />
  );
}

export function useOAuthCalendarSelection(config: CalendarProvider) {
  const auth = useAuth();
  const store = main.UI.useStore(main.STORE_ID);
  const calendars = main.UI.useTable("calendars", main.STORE_ID);
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const { status, scheduleSync, scheduleDebouncedSync, cancelDebouncedSync } =
    useSync();

  const {
    data: incomingCalendars,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ["oauthCalendars", config.id],
    queryFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) return [];
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await googleListCalendars({ client });
      if (error) throw new Error("Failed to fetch calendars");
      return data?.items ?? [];
    },
    enabled: !!auth?.session,
  });

  useEffect(() => {
    if (!incomingCalendars || !store || !user_id) return;

    // The primary calendar's id is the Google account email per the Google Calendar API.
    const primaryCalendarId = incomingCalendars.find((c) => c.primary)?.id;

    store.transaction(() => {
      for (const cal of incomingCalendars) {
        const existingRowId = findCalendarByTrackingId(store, cal.id);
        const rowId = existingRowId ?? crypto.randomUUID();
        const existing = existingRowId
          ? store.getRow("calendars", existingRowId)
          : null;

        store.setRow("calendars", rowId, {
          user_id,
          created_at: existing?.created_at || new Date().toISOString(),
          tracking_id_calendar: cal.id,
          name: cal.summary ?? "Untitled",
          enabled: existing?.enabled ?? false,
          provider: config.id,
          source: primaryCalendarId ?? config.id,
          color: cal.backgroundColor ?? "#4285f4",
        });
      }
    });
  }, [incomingCalendars, store, user_id, config.id]);

  const groups = useMemo((): CalendarGroup[] => {
    const providerCalendars = Object.entries(calendars).filter(
      ([_, cal]) => cal.provider === config.id,
    );

    const grouped = new Map<string, CalendarItem[]>();
    for (const [id, cal] of providerCalendars) {
      const source = cal.source || config.id;
      if (!grouped.has(source)) grouped.set(source, []);
      grouped.get(source)!.push({
        id,
        title: cal.name || "Untitled",
        color: cal.color ?? "#4285f4",
        enabled: cal.enabled ?? false,
      });
    }

    return Array.from(grouped.entries()).map(([sourceName, calendars]) => ({
      sourceName,
      calendars,
    }));
  }, [calendars, config.id]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      store?.setPartialRow("calendars", calendar.id, { enabled });
      scheduleDebouncedSync();
    },
    [store, scheduleDebouncedSync],
  );

  const handleRefresh = useCallback(async () => {
    cancelDebouncedSync();
    await refetch();
    scheduleSync();
  }, [refetch, scheduleSync, cancelDebouncedSync]);

  return {
    groups,
    handleToggle,
    handleRefresh,
    isLoading: isFetching || status === "syncing",
  };
}
