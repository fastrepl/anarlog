import { useMemo } from "react";

import type { SessionEvent } from "@hypr/store";

import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";

export type MentionableEntity = {
  id: string;
  type: "session" | "human" | "organization";
  label: string;
};

export type TimelineEventDto = {
  id: string;
  title: string;
  startedAt: string;
  endedAt: string;
  calendarId: string;
  trackingIdEvent: string;
  hasRecurrenceRules: boolean;
  recurrenceSeriesId: string;
  isAllDay: boolean;
};

export type TimelineSessionDto = {
  id: string;
  title: string;
  createdAt: string;
  folderId: string;
  event: SessionEvent | null;
};

export function useMentionableEntities(): MentionableEntity[] {
  const sessionsTable = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );
  const humansTable = main.UI.useResultTable(
    main.QUERIES.visibleHumans,
    main.STORE_ID,
  );
  const organizationsTable = main.UI.useResultTable(
    main.QUERIES.visibleOrganizations,
    main.STORE_ID,
  );

  return useMemo(() => {
    const results: MentionableEntity[] = [];

    for (const [id, row] of Object.entries(sessionsTable)) {
      const label =
        typeof row.title === "string" && row.title.trim() ? row.title : "";
      if (label) {
        results.push({ id, type: "session", label });
      }
    }

    for (const [id, row] of Object.entries(humansTable)) {
      const label =
        typeof row.name === "string" && row.name.trim() ? row.name : "";
      if (label) {
        results.push({ id, type: "human", label });
      }
    }

    for (const [id, row] of Object.entries(organizationsTable)) {
      const label =
        typeof row.name === "string" && row.name.trim() ? row.name : "";
      if (label) {
        results.push({ id, type: "organization", label });
      }
    }

    return results;
  }, [humansTable, organizationsTable, sessionsTable]);
}

export function useTimelineEventMap(): Record<string, TimelineEventDto> {
  const table = main.UI.useResultTable(
    main.QUERIES.timelineEvents,
    main.STORE_ID,
  );

  return useMemo(() => {
    const out: Record<string, TimelineEventDto> = {};

    for (const [id, row] of Object.entries(table)) {
      out[id] = {
        id,
        title: (row.title as string) ?? "",
        startedAt: (row.started_at as string) ?? "",
        endedAt: (row.ended_at as string) ?? "",
        calendarId: (row.calendar_id as string) ?? "",
        trackingIdEvent: (row.tracking_id_event as string) ?? "",
        hasRecurrenceRules: (row.has_recurrence_rules as boolean) ?? false,
        recurrenceSeriesId: (row.recurrence_series_id as string) ?? "",
        isAllDay: (row.is_all_day as boolean) ?? false,
      };
    }

    return out;
  }, [table]);
}

export function useTimelineSessionMap(): Record<string, TimelineSessionDto> {
  const table = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );

  return useMemo(() => {
    const out: Record<string, TimelineSessionDto> = {};

    for (const [id, row] of Object.entries(table)) {
      const event = getSessionEvent({
        event_json:
          typeof row.event_json === "string" ? row.event_json : undefined,
      });

      out[id] = {
        id,
        title: (row.title as string) ?? "",
        createdAt: (row.created_at as string) ?? "",
        folderId: (row.folder_id as string) ?? "",
        event,
      };
    }

    return out;
  }, [table]);
}
