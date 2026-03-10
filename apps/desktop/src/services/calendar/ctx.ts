import type { Queries } from "tinybase/with-schemas";

import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type {
  CalendarListItem,
  CalendarProviderType,
  ProviderConnectionIds,
} from "@hypr/plugin-calendar";

import { findCalendarByTrackingId } from "~/calendar/utils";
import { QUERIES, type Schemas, type Store } from "~/store/tinybase/store/main";

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

export function createCtx(
  store: Store,
  queries: Queries<Schemas>,
  provider: CalendarProviderType,
  connectionId: string,
): Ctx | null {
  const resultTable = queries.getResultTable(QUERIES.enabledCalendars);

  const calendarIds = new Set<string>();
  const calendarTrackingIdToId = new Map<string, string>();

  for (const calendarId of Object.keys(resultTable)) {
    const calendar = store.getRow("calendars", calendarId);
    if (calendar?.provider !== provider) {
      continue;
    }

    calendarIds.add(calendarId);

    const trackingId = calendar?.tracking_id_calendar as string | undefined;
    if (trackingId) {
      calendarTrackingIdToId.set(trackingId, calendarId);
    }
  }

  // We can't do this because we need a ctx to delete
  // left-over events from old calendars in sync
  // if (calendarTrackingIdToId.size === 0) {
  //   return null;
  // }

  const userId = store.getValue("user_id");
  if (!userId) {
    return null;
  }

  const { from, to } = getRange();

  return {
    store,
    provider,
    connectionId,
    userId: String(userId),
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
  store: Store,
  providerConnections: ProviderConnectionIds[],
): Promise<void> {
  const userId = store.getValue("user_id");
  if (!userId) return;

  for (const { provider, connection_ids } of providerConnections) {
    const incomingCalendars: CalendarListItem[] = [];

    for (const connectionId of connection_ids) {
      const result = await calendarCommands.listCalendars(
        provider,
        connectionId,
      );
      if (result.status === "error") continue;
      incomingCalendars.push(...result.data);
    }

    const incomingIds = new Set(incomingCalendars.map((cal) => cal.id));

    store.transaction(() => {
      for (const rowId of store.getRowIds("calendars")) {
        const row = store.getRow("calendars", rowId);
        if (
          row.provider === provider &&
          !incomingIds.has(row.tracking_id_calendar as string)
        ) {
          store.delRow("calendars", rowId);
        }
      }

      for (const cal of incomingCalendars) {
        const existingRowId = findCalendarByTrackingId(store, cal.id);
        const rowId = existingRowId ?? crypto.randomUUID();
        const existing = existingRowId
          ? store.getRow("calendars", existingRowId)
          : null;

        store.setRow("calendars", rowId, {
          user_id: String(userId),
          created_at: existing?.created_at || new Date().toISOString(),
          tracking_id_calendar: cal.id,
          name: cal.title,
          enabled: existing?.enabled ?? false,
          provider,
          source: cal.source ?? provider,
          color: cal.color ?? "#888",
        });
      }
    });
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
