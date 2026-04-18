import { useCallback } from "react";

import { getSessionEvent, getSessionSearchTimestamp } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";

export type ContactSearchResult = {
  id: string;
  name: string;
  email: string | null;
  jobTitle: string | null;
  organization: string | null;
  memo: string | null;
};

export type CalendarEventSearchResult = {
  id: string;
  title: string;
  startedAt: string | null;
  endedAt: string | null;
  location: string | null;
  meetingLink: string | null;
  description: string | null;
  participantCount: number;
  linkedSessionId: string | null;
};

export function useContactSearchIndex(): (
  query: string,
  limit: number,
) => Promise<ContactSearchResult[]> {
  const humansTable = main.UI.useTable("humans", main.STORE_ID);
  const organizationsTable = main.UI.useTable("organizations", main.STORE_ID);

  return useCallback(
    async (query: string, limit: number) => {
      const q = query.trim().toLowerCase();
      const rows = Object.entries(humansTable)
        .map(([id, row]) => {
          const orgId =
            typeof row.org_id === "string" && row.org_id ? row.org_id : null;
          const orgName =
            orgId &&
            typeof organizationsTable[orgId]?.name === "string" &&
            organizationsTable[orgId]?.name
              ? String(organizationsTable[orgId]?.name)
              : null;

          const name = typeof row.name === "string" ? row.name : "";
          const email =
            typeof row.email === "string" && row.email ? row.email : null;
          const jobTitle =
            typeof row.job_title === "string" && row.job_title
              ? row.job_title
              : null;
          const memo =
            typeof row.memo === "string" && row.memo ? row.memo : null;

          const searchable = [name, email, jobTitle, memo, orgName]
            .filter(Boolean)
            .join("\n")
            .toLowerCase();

          if (q && !searchable.includes(q)) {
            return null;
          }

          return {
            id,
            name,
            email,
            jobTitle,
            organization: orgName,
            memo,
            createdAt: Date.parse((row.created_at as string) || "") || 0,
          };
        })
        .filter(
          (
            row,
          ): row is ContactSearchResult & {
            createdAt: number;
          } => row !== null,
        )
        .sort((a, b) => b.createdAt - a.createdAt)
        .slice(0, limit)
        .map(({ createdAt: _createdAt, ...row }) => row);

      return rows;
    },
    [humansTable, organizationsTable],
  );
}

export function useCalendarEventSearchIndex(): (
  query: string,
  limit: number,
) => Promise<CalendarEventSearchResult[]> {
  const eventsTable = main.UI.useTable("events", main.STORE_ID);
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useCallback(
    async (query: string, limit: number) => {
      const q = query.trim().toLowerCase();
      const sessionByTrackingId = new Map<string, string>();

      for (const [sessionId, row] of Object.entries(sessionsTable)) {
        const event = getSessionEvent({
          event_json:
            typeof row.event_json === "string" ? row.event_json : undefined,
        });
        if (event?.tracking_id) {
          sessionByTrackingId.set(event.tracking_id, sessionId);
        }
      }

      return Object.entries(eventsTable)
        .map(([eventId, row]) => {
          const title = typeof row.title === "string" ? row.title : "";
          const startedAt =
            typeof row.started_at === "string" && row.started_at
              ? row.started_at
              : null;
          const endedAt =
            typeof row.ended_at === "string" && row.ended_at
              ? row.ended_at
              : null;
          const location =
            typeof row.location === "string" && row.location
              ? row.location
              : null;
          const meetingLink =
            typeof row.meeting_link === "string" && row.meeting_link
              ? row.meeting_link
              : null;
          const description =
            typeof row.description === "string" && row.description
              ? row.description
              : null;
          const trackingId =
            typeof row.tracking_id_event === "string"
              ? row.tracking_id_event
              : "";

          let participantCount = 0;
          if (
            typeof row.participants_json === "string" &&
            row.participants_json
          ) {
            try {
              const parsed = JSON.parse(row.participants_json);
              if (Array.isArray(parsed)) {
                participantCount = parsed.length;
              }
            } catch {}
          }

          const searchable = [title, location, meetingLink, description]
            .filter(Boolean)
            .join("\n")
            .toLowerCase();

          if (q && !searchable.includes(q)) {
            return null;
          }

          return {
            id: eventId,
            title: title || "Untitled event",
            startedAt,
            endedAt,
            location,
            meetingLink,
            description,
            participantCount,
            linkedSessionId: sessionByTrackingId.get(trackingId) ?? null,
            startedAtMs: startedAt ? Date.parse(startedAt) || 0 : 0,
          };
        })
        .filter(
          (
            row,
          ): row is CalendarEventSearchResult & {
            startedAtMs: number;
          } => row !== null,
        )
        .sort((a, b) => b.startedAtMs - a.startedAtMs)
        .slice(0, limit)
        .map(({ startedAtMs: _startedAtMs, ...row }) => row);
    },
    [eventsTable, sessionsTable],
  );
}

export function useSessionSearchTimestampLookup(): (
  sessionId: string,
) => number | undefined {
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useCallback(
    (sessionId: string) =>
      getSessionSearchTimestamp(sessionsTable[sessionId] ?? {}),
    [sessionsTable],
  );
}
