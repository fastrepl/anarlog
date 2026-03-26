import { useCallback, useMemo } from "react";

import { useSync } from "../context";

import {
  type CalendarGroup,
  type CalendarItem,
  CalendarSelection,
} from "~/calendar/components/calendar-selection";
import type { CalendarProvider } from "~/calendar/components/shared";
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
  const store = main.UI.useStore(main.STORE_ID);
  const calendars = main.UI.useTable("calendars", main.STORE_ID);
  const { status, scheduleDebouncedSync } = useSync();

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

    const grouped = new Map<string, CalendarItem[]>();

    for (const [id, cal] of providerCalendars) {
      const source =
        cal.source ||
        (cal.connection_id
          ? sourceMap.get(cal.connection_id as string)
          : undefined) ||
        singleSource ||
        config.displayName;
      if (!grouped.has(source)) grouped.set(source, []);
      grouped.get(source)!.push({
        id,
        title: cal.name ?? "Untitled",
        color: cal.color ?? "#4285f4",
        enabled: cal.enabled ?? false,
      });
    }

    return {
      groups: Array.from(grouped.entries()).map(([sourceName, calendars]) => ({
        sourceName,
        calendars,
      })),
      connectionSourceMap: sourceMap,
    };
  }, [calendars, config.id]);

  const handleToggle = useCallback(
    (calendar: CalendarItem, enabled: boolean) => {
      store?.setPartialRow("calendars", calendar.id, { enabled });
      scheduleDebouncedSync();
    },
    [store, scheduleDebouncedSync],
  );

  return {
    groups,
    connectionSourceMap,
    handleToggle,
    isLoading: status === "syncing",
  };
}
