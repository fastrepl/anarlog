import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type {
  CalendarListItem,
  CalendarProviderType,
  ProviderConnectionIds,
} from "@hypr/plugin-calendar";

import {
  deleteCalendar,
  deleteEventsByCalendarId,
  getAllCalendars,
  getEnabledCalendars,
  insertCalendar,
  updateCalendar,
} from "~/calendar/queries";
import {
  findCalendarByTrackingId,
  getCalendarTrackingKey,
} from "~/calendar/utils";
import type { Store } from "~/store/tinybase/store/main";

// ---

export interface Ctx {
  store: Store;
  provider: CalendarProviderType;
  connectionId: string;
  userId: string;
  from: Date;
  to: Date;
  calendarIds: Set<string>;
  calendarTrackingIdToId: Map<string, string>;
}

// ---

export async function createCtx(
  store: Store,
  userId: string,
  provider: CalendarProviderType,
  connectionId: string,
): Promise<Ctx | null> {
  const enabledCalendars = await getEnabledCalendars();

  const calendarIds = new Set<string>();
  const calendarTrackingIdToId = new Map<string, string>();

  for (const calendar of enabledCalendars) {
    if (
      calendar.provider !== provider ||
      calendar.connectionId !== connectionId
    ) {
      continue;
    }

    calendarIds.add(calendar.id);

    if (calendar.trackingIdCalendar) {
      calendarTrackingIdToId.set(calendar.trackingIdCalendar, calendar.id);
    }
  }

  const { from, to } = getRange();

  return {
    store,
    provider,
    connectionId,
    userId,
    from,
    to,
    calendarIds,
    calendarTrackingIdToId,
  };
}

// ---

export async function getProviderConnections(): Promise<
  ProviderConnectionIds[]
> {
  const result = await calendarCommands.listConnectionIds();
  if (result.status === "error") return [];
  return result.data;
}

export async function syncCalendars(
  providerConnections: ProviderConnectionIds[],
): Promise<void> {
  for (const { provider, connection_ids } of providerConnections) {
    const perConnection: {
      connectionId: string;
      calendars: CalendarListItem[];
    }[] = [];

    for (const connectionId of connection_ids) {
      const result = await calendarCommands.listCalendars(
        provider,
        connectionId,
      );
      if (result.status === "error") continue;
      perConnection.push({ connectionId, calendars: result.data });
    }

    const requestedConnectionIds = new Set(connection_ids);
    const successfulConnectionIds = new Set(
      perConnection.map(({ connectionId }) => connectionId),
    );

    const incomingKeys = new Set(
      perConnection.flatMap(({ connectionId, calendars }) =>
        calendars.map((cal) =>
          getCalendarTrackingKey({
            provider,
            connectionId,
            trackingId: cal.id,
          }),
        ),
      ),
    );

    const allCalendars = await getAllCalendars();
    const disabledCalendarIds = new Set<string>();

    for (const cal of allCalendars) {
      if (
        cal.provider === provider &&
        (!requestedConnectionIds.has(cal.connectionId) ||
          (successfulConnectionIds.has(cal.connectionId) &&
            !incomingKeys.has(
              getCalendarTrackingKey({
                provider: cal.provider,
                connectionId: cal.connectionId,
                trackingId: cal.trackingIdCalendar,
              }),
            )))
      ) {
        disabledCalendarIds.add(cal.id);
        await deleteCalendar(cal.id);
      } else if (cal.provider === provider && !cal.enabled) {
        disabledCalendarIds.add(cal.id);
      }
    }

    for (const calId of disabledCalendarIds) {
      await deleteEventsByCalendarId(calId);
    }

    for (const { connectionId, calendars } of perConnection) {
      for (const cal of calendars) {
        const existingRowId = await findCalendarByTrackingId({
          provider,
          connectionId,
          trackingId: cal.id,
        });

        if (existingRowId) {
          await updateCalendar(existingRowId, {
            trackingIdCalendar: cal.id,
            name: cal.title,
            provider,
            source: cal.source ?? "",
            color: cal.color ?? "#888",
            connectionId,
          });
        } else {
          await insertCalendar({
            id: crypto.randomUUID(),
            trackingIdCalendar: cal.id,
            name: cal.title,
            enabled: false,
            provider,
            source: cal.source ?? "",
            color: cal.color ?? "#888",
            connectionId,
          });
        }
      }
    }
  }
}

// ---

const getRange = () => {
  const now = new Date();
  const from = new Date(now);
  from.setDate(from.getDate() - 7);
  const to = new Date(now);
  to.setDate(to.getDate() + 30);
  return { from, to };
};
