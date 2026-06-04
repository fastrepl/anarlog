import type { Queries } from "tinybase/with-schemas";

import type { CalendarProviderType } from "@hypr/plugin-calendar";

import {
  type CalendarSyncRange,
  createCtx,
  getProviderConnections,
  syncCalendars,
} from "./ctx";
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
export type { CalendarSyncRange };
type CalendarSyncOptions = {
  signal?: AbortSignal;
};

export async function syncCalendarEvents(
  store: Store,
  queries: Queries<Schemas>,
): Promise<void> {
  await Promise.all([
    new Promise((resolve) => setTimeout(resolve, 250)),
    run(store, queries),
  ]);
}

export async function syncCalendarEventsForRange(
  store: Store,
  queries: Queries<Schemas>,
  range: CalendarSyncRange,
  options: CalendarSyncOptions = {},
): Promise<void> {
  await run(store, queries, range, options);
}

async function run(
  store: Store,
  queries: Queries<Schemas>,
  range?: CalendarSyncRange,
  options: CalendarSyncOptions = {},
) {
  if (isAborted(options.signal)) return;

  const providerConnections = await getProviderConnections();
  if (isAborted(options.signal)) return;

  await syncCalendars(store, providerConnections);
  if (isAborted(options.signal)) return;

  for (const { provider, connection_ids } of providerConnections) {
    for (const connectionId of connection_ids) {
      if (isAborted(options.signal)) return;

      try {
        await runForConnection(
          store,
          queries,
          provider,
          connectionId,
          range,
          options,
        );
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
  range?: CalendarSyncRange,
  options: CalendarSyncOptions = {},
) {
  const ctx = createCtx(store, queries, provider, connectionId, range);
  if (!ctx || isAborted(options.signal)) {
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

  if (isAborted(options.signal)) return;

  const existing = fetchExistingEvents(ctx);
  if (isAborted(options.signal)) return;

  const eventsOut = syncEvents(ctx, {
    incoming,
    existing,
    incomingParticipants,
  });
  if (isAborted(options.signal)) return;

  executeForEventsSync(ctx, eventsOut);
  if (isAborted(options.signal)) return;

  syncSessionEmbeddedEvents(ctx, incoming);
  if (isAborted(options.signal)) return;

  const participantsOut = syncSessionParticipants(ctx, {
    incomingParticipants,
  });
  if (isAborted(options.signal)) return;

  executeForParticipantsSync(ctx, participantsOut);
}

function isAborted(signal: AbortSignal | undefined) {
  return signal?.aborted === true;
}
