import type { Queries } from "tinybase/with-schemas";

import type { CalendarProviderType } from "@hypr/plugin-calendar";

import { createCtx, getProviderConnections, syncCalendars } from "./ctx";
import {
  CalendarFetchError,
  fetchExistingEvents,
  fetchIncomingEvents,
} from "./fetch";
import {
  executeForEventsSync,
  executeForParticipantsSync,
  syncEvents,
  syncSessionEmbeddedEvents,
  syncSessionParticipants,
} from "./process";

import type { Schemas, Store } from "~/store/tinybase/store/main";

export const CALENDAR_SYNC_TASK_ID = "calendarSync";

export async function syncCalendarEvents(
  store: Store,
  queries: Queries<Schemas>,
): Promise<void> {
  await Promise.all([
    new Promise((resolve) => setTimeout(resolve, 250)),
    run(store, queries),
  ]);
}

async function run(store: Store, queries: Queries<Schemas>) {
  const providerConnections = await getProviderConnections();
  await syncCalendars(store, providerConnections);
  for (const { provider, connection_ids } of providerConnections) {
    for (const connectionId of connection_ids) {
      try {
        await runForConnection(store, queries, provider, connectionId);
      } catch (error) {
        console.error(
          `[calendar-sync] Error syncing ${provider} (${connectionId}): ${error}`,
        );
      }
    }
  }
}

async function runForConnection(
  store: Store,
  queries: Queries<Schemas>,
  provider: CalendarProviderType,
  connectionId: string,
) {
  const ctx = createCtx(store, queries, provider, connectionId);
  if (!ctx) {
    return;
  }

  let incoming;
  let incomingParticipants;

  try {
    const result = await fetchIncomingEvents(ctx);
    incoming = result.events;
    incomingParticipants = result.participants;
  } catch (error) {
    if (error instanceof CalendarFetchError) {
      console.error(
        `[calendar-sync] Aborting ${provider} sync due to fetch error: ${error.message}`,
      );
      return;
    }
    throw error;
  }

  const existing = fetchExistingEvents(ctx);

  const eventsOut = syncEvents(ctx, {
    incoming,
    existing,
    incomingParticipants,
  });
  executeForEventsSync(ctx, eventsOut);
  syncSessionEmbeddedEvents(ctx, incoming);

  const participantsOut = syncSessionParticipants(ctx, {
    incomingParticipants,
  });
  executeForParticipantsSync(ctx, participantsOut);
}
