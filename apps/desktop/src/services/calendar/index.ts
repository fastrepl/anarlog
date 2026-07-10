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
  syncEvents,
  syncSessionEmbeddedEvents,
  syncSessionParticipants,
} from "./process";
import {
  applyConnectionSync,
  loadParticipantSyncSnapshot,
  loadSessionsForTrackingIds,
} from "./storage";

import { enqueueDatabaseWrite } from "~/db/write-queue";

export const CALENDAR_SYNC_TASK_ID = "calendarSync";
export type { CalendarSyncRange };

type CalendarSyncOptions = {
  signal?: AbortSignal;
};

export function syncCalendarEvents(): Promise<void> {
  return enqueueDatabaseWrite("calendar-sync", async () => {
    await Promise.all([
      new Promise((resolve) => setTimeout(resolve, 250)),
      run(),
    ]);
  });
}

export function syncCalendarEventsForRange(
  range: CalendarSyncRange,
  options: CalendarSyncOptions = {},
): Promise<void> {
  return enqueueDatabaseWrite("calendar-sync", () => run(range, options));
}

async function run(
  range?: CalendarSyncRange,
  options: CalendarSyncOptions = {},
) {
  if (isAborted(options.signal)) return;

  const providerConnections = await getProviderConnections();
  if (isAborted(options.signal)) return;

  await syncCalendars(providerConnections);
  if (isAborted(options.signal)) return;

  for (const { provider, connection_ids } of providerConnections) {
    for (const connectionId of connection_ids) {
      if (isAborted(options.signal)) return;

      try {
        await runForConnection(provider, connectionId, range, options);
      } catch (error) {
        console.error(
          `[calendar-sync] Error syncing ${provider} (${connectionId}): ${error}`,
        );
      }
    }
  }
}

async function runForConnection(
  provider: CalendarProviderType,
  connectionId: string,
  range?: CalendarSyncRange,
  options: CalendarSyncOptions = {},
) {
  const ctx = await createCtx(provider, connectionId, range);
  if (isAborted(options.signal)) return;

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

  const existing = await fetchExistingEvents(ctx, incoming);
  if (isAborted(options.signal)) return;

  const events = syncEvents(ctx, {
    incoming,
    existing,
    incomingParticipants,
  });
  const sessions = await loadSessionsForTrackingIds(
    incoming.map((event) => event.tracking_id_event),
  );
  if (isAborted(options.signal)) return;

  const sessionUpdates = syncSessionEmbeddedEvents(ctx, incoming, sessions);
  const participantSnapshot = await loadParticipantSyncSnapshot(
    sessions,
    incomingParticipants,
  );
  if (isAborted(options.signal)) return;

  const participants = syncSessionParticipants({
    incomingParticipants,
    snapshot: participantSnapshot,
  });
  await applyConnectionSync({
    ctx,
    events,
    sessionUpdates,
    participants,
  });
}

function isAborted(signal: AbortSignal | undefined) {
  return signal?.aborted === true;
}
