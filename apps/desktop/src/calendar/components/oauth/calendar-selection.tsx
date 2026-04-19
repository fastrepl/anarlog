import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import type { CalendarProvider } from "~/calendar/components/shared";
import { useSetCalendarEnabled } from "~/calendar/hooks";
import * as main from "~/store/tinybase/store/main";

export function OAuthCalendarSelection({
  groups,
  onToggle,
  onRefresh,
  isLoading,
}: {
  groups: CalendarGroup[];
  onToggle: (calendar: CalendarItem, enabled: boolean) => void;
  onRefresh?: () => void;
  isLoading: boolean;
}) {
  return (
    <CalendarSelection
      groups={groups}
      onToggle={onToggle}
      onRefresh={onRefresh}
      isLoading={isLoading}
    />
  );
}

export function useOAuthCalendarSelection(config: CalendarProvider) {
  const queryClient = useQueryClient();
  const setCalendarEnabled = useSetCalendarEnabled();
  const calendars = main.UI.useTable("calendars", main.STORE_ID);
  const { cancelDebouncedSync, status, scheduleDebouncedSync, scheduleSync } =
    useSync();

  const { groups, connectionSourceMap } = useMemo(() => {
    const providerCalendars = Object.entries(calendars).filter(
      ([_, cal]) => cal.provider === config.id,
    );

    const sourceMap = new Map<string, string>();

    for (const [_, cal] of providerCalendars) {
      // HACK: derive connection_id -> source mapping from calendar entries
      if (cal.source && cal.connection_id) {
        sourceMap.set(cal.connection_id as string, cal.source as string);
      }
    }

    const nonNullSources = new Set(
      providerCalendars
        .map(([_, cal]) => {
          if (cal.source) {
            return cal.source;
          }
          if (cal.connection_id) {
            return sourceMap.get(cal.connection_id as string);
          }
          return undefined;
        })
        .filter(Boolean),
    );
    const singleSource =
      nonNullSources.size === 1 ? ([...nonNullSources][0] as string) : null;

    const grouped = new Map<
      string,
      { connectionId?: string; calendars: CalendarItem[] }
    >();

    for (const [id, cal] of providerCalendars) {
      const connectionId =
        typeof cal.connection_id === "string" ? cal.connection_id : undefined;
      const source =
        cal.source ||
        (connectionId ? sourceMap.get(connectionId) : undefined) ||
        singleSource ||
        config.displayName;
      if (!grouped.has(source)) {
        grouped.set(source, { connectionId, calendars: [] });
      }
      const group = grouped.get(source)!;
      if (!group.connectionId && connectionId) {
        group.connectionId = connectionId;
      }
      group.calendars.push({
        id,
        title: cal.name ?? "Untitled",
        color: cal.color ?? "#4285f4",
        enabled: cal.enabled ?? false,
      });
    }

    return {
      groups: Array.from(grouped.entries()).map(([sourceName, group]) => ({
        id: group.connectionId,
        sourceName,
        calendars: group.calendars,
      })),
      connectionSourceMap: sourceMap,
    };
  }, [calendars, config.id]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      void setCalendarEnabled(calendar.id, enabled).catch((error) => {
        console.error("[oauth-calendar-selection] setCalendarEnabled:", error);
      });
      scheduleDebouncedSync();
    },
    [setCalendarEnabled, scheduleDebouncedSync],
  );

  const handleRefresh = useCallback(() => {
    cancelDebouncedSync();
    void queryClient.invalidateQueries({
      queryKey: ["integration-status"],
    });
    scheduleSync();
  }, [cancelDebouncedSync, queryClient, scheduleSync]);

  return {
    groups,
    connectionSourceMap,
    handleRefresh,
    handleToggle,
    isLoading: status === "syncing",
  };
}
