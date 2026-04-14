import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import type { CalendarProvider } from "~/calendar/components/shared";
import { useCalendars, useToggleCalendarEnabled } from "~/calendar/queries";

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
  const allCalendars = useCalendars();
  const toggleEnabled = useToggleCalendarEnabled();
  const { cancelDebouncedSync, status, scheduleDebouncedSync, scheduleSync } =
    useSync();

  const { groups, connectionSourceMap } = useMemo(() => {
    const providerCalendars = allCalendars.filter(
      (cal) => cal.provider === config.id,
    );

    const sourceMap = new Map<string, string>();

    for (const cal of providerCalendars) {
      if (cal.source && cal.connectionId) {
        sourceMap.set(cal.connectionId, cal.source);
      }
    }

    const nonNullSources = new Set(
      providerCalendars
        .map((cal) => {
          if (cal.source) {
            return cal.source;
          }
          if (cal.connectionId) {
            return sourceMap.get(cal.connectionId);
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

    for (const cal of providerCalendars) {
      const connectionId = cal.connectionId || undefined;
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
  }, [allCalendars, config.id]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      void toggleEnabled(calendar.id, enabled);
      scheduleDebouncedSync();
    },
    [toggleEnabled, scheduleDebouncedSync],
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
