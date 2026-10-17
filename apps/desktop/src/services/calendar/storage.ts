import {
  commands as calendarCommands,
  type CalendarListItem,
  type CalendarProviderType,
} from "@anlg/plugin-calendar";

import type { Ctx } from "./ctx";
import type { IncomingEvent, IncomingParticipants } from "./fetch/types";

import { liveQueryClient } from "~/db";

type CalendarSqlRow = {
  id: string;
  tracking_id_calendar: string;
  name: string;
  enabled: boolean | number;
  provider: string;
  source: string;
  color: string;
  connection_id: string;
  created_at: string;
  deleted_at: string | null;
};

export type StoredCalendar = Omit<CalendarSqlRow, "enabled"> & {
  enabled: boolean;
};

export async function loadEnabledCalendars(
  provider: CalendarProviderType,
  connectionId: string,
): Promise<StoredCalendar[]> {
  const rows = await liveQueryClient.execute<CalendarSqlRow>(
    `
      SELECT
        id,
        tracking_id_calendar,
        name,
        enabled,
        provider,
        source,
        color,
        connection_id,
        created_at,
        deleted_at
      FROM calendars
      WHERE provider = ?
        AND connection_id = ?
        AND enabled = 1
        AND deleted_at IS NULL
      ORDER BY created_at, id
    `,
    [provider, connectionId],
  );

  return rows.map(normalizeCalendar);
}

export async function applyCalendarInventory({
  provider,
  requestedConnectionIds,
  successfulConnections,
}: {
  provider: CalendarProviderType;
  requestedConnectionIds: string[];
  successfulConnections: Array<{
    connectionId: string;
    calendars: CalendarListItem[];
  }>;
}): Promise<void> {
  const result = await calendarCommands.applyCalendarInventory({
    provider,
    requested_connection_ids: requestedConnectionIds,
    successful_connections: successfulConnections.map(
      ({ connectionId, calendars }) => ({
        connection_id: connectionId,
        calendars,
      }),
    ),
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }
}

export async function tombstoneCalendarConnection(
  provider: CalendarProviderType,
  connectionId: string,
): Promise<void> {
  const result = await calendarCommands.tombstoneCalendarConnection({
    provider,
    connection_id: connectionId,
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }
}

export async function syncConnectionEvents({
  ctx,
  incoming,
  incomingParticipants,
}: {
  ctx: Ctx;
  incoming: IncomingEvent[];
  incomingParticipants: IncomingParticipants;
}): Promise<void> {
  const result = await calendarCommands.syncCalendarConnectionEvents({
    provider: ctx.provider,
    connection_id: ctx.connectionId,
    from: ctx.from.toISOString(),
    to: ctx.to.toISOString(),
    calendars: ctx.calendars,
    events: incoming,
    participants: Array.from(incomingParticipants, ([key, participants]) => ({
      tracking_id_event: key,
      participants,
    })),
  });
  if (result.status === "error") {
    throw new Error(result.error);
  }
}

function normalizeCalendar(row: CalendarSqlRow): StoredCalendar {
  return { ...row, enabled: Boolean(row.enabled) };
}
