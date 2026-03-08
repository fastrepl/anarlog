import type { Queries } from "tinybase/with-schemas";

import type { CalendarProviderType } from "@hypr/plugin-calendar";

import { createCtx, getActiveProviders, syncCalendars } from "./ctx";
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
  const providers = await getActiveProviders();
  await syncCalendars(store, providers);
  for (const provider of providers) {
    try {
      await runForProvider(store, queries, provider);
    } catch (error) {
      console.error(`[calendar-sync] Error syncing ${provider}: ${error}`);
    }
  }
}

async function runForProvider(
  store: Store,
  queries: Queries<Schemas>,
  provider: CalendarProviderType,
) {
  const ctx = createCtx(store, queries, provider);
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
