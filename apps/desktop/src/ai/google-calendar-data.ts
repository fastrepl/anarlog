import { liveQueryClient } from "~/db";

export type GoogleCalendarDataSqlRow = {
  has_google_calendar_data: boolean | number;
};

export const GOOGLE_CALENDAR_DATA_SQL = `
  SELECT (
    EXISTS (
      SELECT 1
      FROM calendars
      WHERE provider IN ('google', 'google-calendar')
    )
    OR EXISTS (
      SELECT 1
      FROM events
      WHERE provider IN ('google', 'google-calendar')
    )
    OR EXISTS (
      SELECT 1
      FROM sessions AS session
      WHERE session.external_provider IN ('google', 'google-calendar')
        OR (
          session.external_provider = ''
          AND (
            session.event_id <> ''
            OR session.external_event_id <> ''
            OR (
              LOWER(TRIM(session.event_json)) NOT IN ('', '{}', 'null')
              AND NOT (
                json_valid(session.event_json)
                AND json_extract(session.event_json, '$.tracking_id') = 'anarlog-onboarding-demo-v1'
              )
            )
          )
          AND NOT EXISTS (
            SELECT 1
            FROM events AS linked_non_google_event
            WHERE (
              (
                session.event_id <> ''
                AND linked_non_google_event.id = session.event_id
              )
              OR (
                session.external_event_id <> ''
                AND json_valid(session.event_json)
                AND COALESCE(json_extract(session.event_json, '$.calendar_id'), '') <> ''
                AND linked_non_google_event.tracking_id_event = session.external_event_id
                AND linked_non_google_event.calendar_id = json_extract(session.event_json, '$.calendar_id')
              )
            )
            AND linked_non_google_event.provider IN ('apple', 'outlook')
          )
          AND NOT EXISTS (
            SELECT 1
            FROM calendars AS linked_non_google_calendar
            WHERE json_valid(session.event_json)
              AND COALESCE(json_extract(session.event_json, '$.calendar_id'), '') <> ''
              AND linked_non_google_calendar.id = json_extract(session.event_json, '$.calendar_id')
              AND linked_non_google_calendar.provider IN ('apple', 'outlook')
          )
        )
    )
  ) AS has_google_calendar_data
`;

export function mapGoogleCalendarDataRows(
  rows: GoogleCalendarDataSqlRow[],
): boolean {
  const row = rows[0];
  if (!row) {
    throw new Error("Google Calendar data boundary query returned no result");
  }
  return Boolean(row.has_google_calendar_data);
}

export async function hasGoogleCalendarData(): Promise<boolean> {
  const rows = await liveQueryClient.execute<GoogleCalendarDataSqlRow>(
    GOOGLE_CALENDAR_DATA_SQL,
    [],
  );
  return mapGoogleCalendarDataRows(rows);
}
