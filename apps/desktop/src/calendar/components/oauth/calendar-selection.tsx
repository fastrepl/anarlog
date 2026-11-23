import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import type { ConnectionItem } from "@anlg/api-client";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import type { CalendarProvider } from "~/calendar/components/shared";
import { setCalendarEnabled, useCalendarRows } from "~/calendar/queries";
import {
  useCalendarSyncIssues,
  connectionKey,
} from "~/services/calendar/sync-errors";

export function OAuthCalendarSelection({
  groups,
  onToggle,
  onRefresh,
  isLoading,
}: {
  groups: CalendarGroup[];
  onToggle: (
    calendar: CalendarItem,
    enabled: boolean,
  ) => void | Promise<unknown>;
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

export function useOAuthCalendarSelection(
  config: CalendarProvider,
  connections: ConnectionItem[],
) {
  const issues = useCalendarSyncIssues();
  const queryClient = useQueryClient();
  const calendars = useCalendarRows(config.id);
  const { cancelDebouncedSync, status, scheduleDebouncedSync, scheduleSync } =
    useSync();

  const groups = useMemo(() => {
    const sourceByConnection = new Map<string, string>();
    const ownerBySource = new Map<string, string | undefined>();
    const sources = new Set<string>();
    for (const cal of calendars) {
      if (!cal.source) continue;
      sources.add(cal.source);
      if (!cal.connection_id) continue;
      sourceByConnection.set(cal.connection_id, cal.source);
      if (!ownerBySource.has(cal.source))
        ownerBySource.set(cal.source, cal.connection_id);
      else if (ownerBySource.get(cal.source) !== cal.connection_id)
        ownerBySource.set(cal.source, undefined);
    }
    const singleSource =
      sources.size === 1 ? sources.values().next().value : undefined;

    const grouped = new Map<string, CalendarGroup>();
    for (const cal of calendars) {
      const source =
        cal.source ||
        (cal.connection_id && sourceByConnection.get(cal.connection_id)) ||
        singleSource ||
        config.displayName;
      const connectionId = cal.connection_id || ownerBySource.get(source);
      const key = JSON.stringify([connectionId, source]);
      const group: CalendarGroup = grouped.get(key) ?? {
        id: connectionId,
        sourceName: source,
        calendars: [],
      };
      grouped.set(key, group);
      group.calendars.push({
        id: cal.id,
        title: cal.name ?? "Untitled",
        color: cal.color ?? "#4285f4",
        enabled: cal.enabled ?? false,
      });
    }

    const groups = Array.from(grouped.values());
    for (const connection of connections) {
      const id = connection.connection_id;
      const issue = issues[connectionKey(config.id, id)];
      if (!issue) continue;
      const account = connection.account_identity;
      const group =
        groups.find((item) => item.id === id && item.sourceName === account) ??
        groups.find((item) => item.id === id);
      if (group) group.issue = issue;
      else
        groups.push({
          id,
          sourceName: connection.account_identity || config.displayName,
          calendars: [],
          issue,
        });
    }
    return groups;
  }, [calendars, config.id, config.displayName, connections, issues]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) =>
      setCalendarEnabled(calendar.id, enabled)
        .then(scheduleDebouncedSync)
        .catch((error: unknown) => {
          console.error("[calendar] failed to update calendar", error);
          throw error;
        }),
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
    handleRefresh,
    handleToggle,
    isLoading: status === "syncing",
  };
}
