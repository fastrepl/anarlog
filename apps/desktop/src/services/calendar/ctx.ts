import { commands as calendarCommands } from "@anlg/plugin-calendar";
import type {
  CalendarListItem,
  CalendarProviderType,
  ProviderConnectionIds,
} from "@anlg/plugin-calendar";

import { applyCalendarInventory, loadEnabledCalendars } from "./storage";
import {
  calendarSyncIssue,
  connectionKey,
  type CalendarSyncIssue,
} from "./sync-errors";

import { enqueueDatabaseWrite } from "~/db/write-queue";

export interface Ctx {
  provider: CalendarProviderType;
  connectionId: string;
  from: Date;
  to: Date;
  calendars: Array<{ id: string; tracking_id_calendar: string }>;
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
  return {
    provider,
    connectionId,
    from: range.from,
    to: range.to,
    calendars: calendars.map((calendar) => ({
      id: calendar.id,
      tracking_id_calendar: calendar.tracking_id_calendar,
    })),
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
  shouldStop: () => boolean = () => signal?.aborted === true,
) {
  const failures = new Map<string, CalendarSyncIssue>();
  for (const { provider, connection_ids } of providerConnections) {
    if (shouldStop()) break;

    const successfulConnections: Array<{
      connectionId: string;
      calendars: CalendarListItem[];
    }> = [];

    for (const connectionId of connection_ids) {
      if (shouldStop()) return failures;

      try {
        const result = await calendarCommands.listCalendars(
          provider,
          connectionId,
        );
        if (result.status === "error") throw result.error;
        successfulConnections.push({ connectionId, calendars: result.data });
      } catch (error) {
        failures.set(
          connectionKey(provider, connectionId),
          calendarSyncIssue(error),
        );
      }
    }

    if (shouldStop()) break;
    await enqueueDatabaseWrite("calendar-sync", async () => {
      if (shouldStop()) return;
      await applyCalendarInventory({
        provider,
        requestedConnectionIds: connection_ids,
        successfulConnections,
      });
    });
  }
  return failures;
}

const getDefaultRange = (): CalendarSyncRange => {
  const now = new Date();
  const from = new Date(now);
  from.setHours(0, 0, 0, 0);
  from.setDate(from.getDate() - 6);
  const to = new Date(now);
  to.setHours(0, 0, 0, 0);
  to.setDate(to.getDate() + 2);
  return { from, to };
};
