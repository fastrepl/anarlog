import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import type { CalendarProvider } from "~/calendar/components/shared";
import { setCalendarEnabled, useCalendarRows } from "~/calendar/queries";

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
  const calendars = useCalendarRows(config.id);
  const { cancelDebouncedSync, status, scheduleDebouncedSync, scheduleSync } =
    useSync();

  const { groups, connectionSourceMap } = useMemo(() => {
    const sourceMap = new Map<string, string>();

    for (const cal of calendars) {
      if (cal.source && cal.connection_id) {
        sourceMap.set(cal.connection_id, cal.source);
      }
    }

    const nonNullSources = new Set(
      calendars
        .map((cal) => {
          if (cal.source) {
            return cal.source;
          }
          if (cal.connection_id) {
            return sourceMap.get(cal.connection_id);
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

    for (const cal of calendars) {
      const connectionId = cal.connection_id || undefined;
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
        id: cal.id,
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
  }, [calendars, config.displayName]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      void setCalendarEnabled(calendar.id, enabled)
        .then(scheduleDebouncedSync)
        .catch((error) => {
          console.error("[calendar] failed to update calendar", error);
        });
    },
    [scheduleDebouncedSync],
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
