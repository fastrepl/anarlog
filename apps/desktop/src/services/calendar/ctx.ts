import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type {
  CalendarListItem,
  CalendarProviderType,
  ProviderConnectionIds,
} from "@hypr/plugin-calendar";

import { applyCalendarInventory, loadEnabledCalendars } from "./storage";

export interface Ctx {
  provider: CalendarProviderType;
  connectionId: string;
  from: Date;
  to: Date;
  calendarIds: Set<string>;
  calendarTrackingIdToId: Map<string, string>;
}

export type CalendarSyncRange = {
  from: Date;
  to: Date;
};

export async function createCtx(
  provider: CalendarProviderType,
  connectionId: string,
  range: CalendarSyncRange = getDefaultRange(),
): Promise<Ctx> {
  const calendars = await loadEnabledCalendars(provider, connectionId);
  const calendarIds = new Set<string>();
  const calendarTrackingIdToId = new Map<string, string>();

  for (const calendar of calendars) {
    calendarIds.add(calendar.id);
    if (calendar.tracking_id_calendar) {
      calendarTrackingIdToId.set(calendar.tracking_id_calendar, calendar.id);
    }
  }

  return {
    provider,
    connectionId,
    from: range.from,
    to: range.to,
    calendarIds,
    calendarTrackingIdToId,
  };
}

export async function getProviderConnections(): Promise<
  ProviderConnectionIds[]
> {
  const result = await calendarCommands.listConnectionIds();
  if (result.status === "error") {
    throw new Error(`Failed to discover calendar connections: ${result.error}`);
  }
  return result.data;
}

export async function syncCalendars(
  providerConnections: ProviderConnectionIds[],
  signal?: AbortSignal,
): Promise<void> {
  for (const { provider, connection_ids } of providerConnections) {
    if (signal?.aborted) return;

    const successfulConnections: Array<{
      connectionId: string;
      calendars: CalendarListItem[];
    }> = [];

    for (const connectionId of connection_ids) {
      if (signal?.aborted) return;

      const result = await calendarCommands.listCalendars(
        provider,
        connectionId,
      );
      if (signal?.aborted) return;
      if (result.status === "error") continue;
      successfulConnections.push({ connectionId, calendars: result.data });
    }

    if (signal?.aborted) return;
    await applyCalendarInventory({
      provider,
      requestedConnectionIds: connection_ids,
      successfulConnections,
    });
  }
}

export const getDefaultRange = (): CalendarSyncRange => {
  const now = new Date();
  const from = new Date(now);
  from.setHours(0, 0, 0, 0);
  from.setDate(from.getDate() - 6);
  const to = new Date(now);
  to.setHours(0, 0, 0, 0);
  to.setDate(to.getDate() + 2);
  return { from, to };
};
