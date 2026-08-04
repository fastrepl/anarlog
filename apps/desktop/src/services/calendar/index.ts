import type { Manager } from "tinytick";

import type { CalendarProviderType } from "@anlg/plugin-calendar";

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
  tombstoneCalendarConnection,
} from "./storage";

import { enqueueDatabaseWrite } from "~/db/write-queue";

export const CALENDAR_SYNC_TASK_ID = "calendarSync";
export type { CalendarSyncRange };

type CalendarSyncOptions = {
  signal?: AbortSignal;
};

let calendarSyncTail: Promise<void> = Promise.resolve();
let calendarSyncGeneration = 0;
const disconnectedCalendarConnections = new Set<string>();

function enqueueCalendarSync(sync: () => Promise<void>): Promise<void> {
  const result = calendarSyncTail.catch(() => undefined).then(sync);
  calendarSyncTail = result.catch(() => undefined);
  return result;
}

export function syncCalendarEvents(
  options: CalendarSyncOptions = {},
): Promise<void> {
  return enqueueCalendarSync(async () => {
    const generation = calendarSyncGeneration;
    await Promise.all([
      new Promise((resolve) => setTimeout(resolve, 250)),
      run(undefined, options, generation),
    ]);
  });
}

export function scheduleCalendarSync(manager: Manager): string | undefined {
  const activeTaskRunId = [
    ...manager.getScheduledTaskRunIds(),
    ...manager.getRunningTaskRunIds(),
  ].find(
    (taskRunId) =>
      manager.getTaskRunInfo(taskRunId)?.taskId === CALENDAR_SYNC_TASK_ID,
  );

  return activeTaskRunId ?? manager.scheduleTaskRun(CALENDAR_SYNC_TASK_ID);
}

export function syncCalendarEventsForRange(
  range: CalendarSyncRange,
  options: CalendarSyncOptions = {},
): Promise<void> {
  return enqueueCalendarSync(() => run(range, options, calendarSyncGeneration));
}

export function removeDisconnectedCalendarConnection(
  integrationId: string,
  connectionId: string,
): Promise<void> {
  const provider = calendarProviderForIntegration(integrationId);

  if (!provider) return Promise.resolve();

  const key = connectionKey(provider, connectionId);
  calendarSyncGeneration += 1;
  disconnectedCalendarConnections.add(key);
  return enqueueDatabaseWrite("calendar-sync", () =>
    tombstoneCalendarConnection(provider, connectionId),
  ).catch((error) => {
    disconnectedCalendarConnections.delete(key);
    throw error;
  });
}

export function allowReconnectedCalendarConnections(
  integrationId: string,
): void {
  const provider = calendarProviderForIntegration(integrationId);
  if (!provider) return;

  calendarSyncGeneration += 1;
  const prefix = `${provider}:`;
  for (const key of disconnectedCalendarConnections) {
    if (key.startsWith(prefix)) disconnectedCalendarConnections.delete(key);
  }
}

async function run(
  range?: CalendarSyncRange,
  options: CalendarSyncOptions = {},
  generation = calendarSyncGeneration,
) {
  const shouldStop = () => isStopped(options.signal, generation);
  if (shouldStop()) return;

  const discoveredConnections = await getProviderConnections();
  if (shouldStop()) return;
  const providerConnections = excludeDisconnectedConnections(
    discoveredConnections,
  );

  await syncCalendars(providerConnections, options.signal, shouldStop);
  if (shouldStop()) return;

  for (const { provider, connection_ids } of providerConnections) {
    for (const connectionId of connection_ids) {
      if (shouldStop()) return;

      try {
        await runForConnection(
          provider,
          connectionId,
          range,
          options,
          generation,
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
  provider: CalendarProviderType,
  connectionId: string,
  range?: CalendarSyncRange,
  options: CalendarSyncOptions = {},
  generation = calendarSyncGeneration,
) {
  const shouldStop = () => isStopped(options.signal, generation);
  const ctx = await createCtx(provider, connectionId, range);
  if (shouldStop()) return;

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

  if (shouldStop()) return;

  const existing = await fetchExistingEvents(ctx, incoming);
  if (shouldStop()) return;

  const events = syncEvents(ctx, {
    incoming,
    existing,
    incomingParticipants,
  });
  const sessions = await loadSessionsForTrackingIds(
    incoming.map((event) => event.tracking_id_event),
  );
  if (shouldStop()) return;

  const sessionUpdates = syncSessionEmbeddedEvents(ctx, incoming, sessions);
  const participantSnapshot = await loadParticipantSyncSnapshot(
    sessions,
    incomingParticipants,
  );
  if (shouldStop()) return;

  const participants = syncSessionParticipants({
    incomingParticipants,
    snapshot: participantSnapshot,
  });
  await enqueueDatabaseWrite("calendar-sync", async () => {
    if (shouldStop()) return;
    await applyConnectionSync({
      ctx,
      events,
      sessionUpdates,
      participants,
    });
  });
}

function isStopped(signal: AbortSignal | undefined, generation: number) {
  return signal?.aborted === true || generation !== calendarSyncGeneration;
}

function excludeDisconnectedConnections(
  providerConnections: Awaited<ReturnType<typeof getProviderConnections>>,
) {
  const discoveredConnectionKeys = new Set(
    providerConnections.flatMap(({ provider, connection_ids }) =>
      connection_ids.map((connectionId) =>
        connectionKey(provider, connectionId),
      ),
    ),
  );
  for (const key of disconnectedCalendarConnections) {
    if (!discoveredConnectionKeys.has(key)) {
      disconnectedCalendarConnections.delete(key);
    }
  }

  return providerConnections.flatMap(({ provider, connection_ids }) => {
    const activeConnectionIds = connection_ids.filter(
      (connectionId) =>
        !disconnectedCalendarConnections.has(
          connectionKey(provider, connectionId),
        ),
    );
    return activeConnectionIds.length > 0
      ? [{ provider, connection_ids: activeConnectionIds }]
      : [];
  });
}

function connectionKey(
  provider: CalendarProviderType,
  connectionId: string,
): string {
  return `${provider}:${connectionId}`;
}

function calendarProviderForIntegration(
  integrationId: string,
): CalendarProviderType | null {
  return integrationId === "google-calendar"
    ? "google"
    : integrationId === "outlook"
      ? "outlook"
      : null;
}
