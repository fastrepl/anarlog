import type {
  CalendarProviderType,
  ProviderConnectionIds,
} from "@hypr/plugin-calendar";

import type { Ctx } from "./ctx";
import { getProviderConnections } from "./ctx";
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

export const CALENDAR_SYNC_TASK_ID = "calendarSync";

export interface CalendarSyncDependencies {
  createCtx: (
    provider: CalendarProviderType,
    connectionId: string,
  ) => Ctx | null;
  syncCalendars: (
    providerConnections: ProviderConnectionIds[],
  ) => Promise<void>;
}

export async function syncCalendarEvents(
  deps: CalendarSyncDependencies,
): Promise<void> {
  await Promise.all([
    new Promise((resolve) => setTimeout(resolve, 250)),
    run(deps),
  ]);
}

async function run(deps: CalendarSyncDependencies) {
  const providerConnections = await getProviderConnections();
  await deps.syncCalendars(providerConnections);
  for (const { provider, connection_ids } of providerConnections) {
    for (const connectionId of connection_ids) {
      try {
        await runForConnection(deps, provider, connectionId);
      } catch (error) {
        console.error(
          `[calendar-sync] Error syncing ${provider} (${connectionId}): ${error}`,
        );
      }
    }
  }
}

async function runForConnection(
  deps: CalendarSyncDependencies,
  provider: CalendarProviderType,
  connectionId: string,
) {
  const ctx = deps.createCtx(provider, connectionId);
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
