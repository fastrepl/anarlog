import type { CalendarProviderType } from "@anlg/plugin-calendar";

import {
  type CalendarSyncRange,
  createCtx,
  getProviderConnections,
  syncCalendars,
} from "./ctx";
import { fetchIncomingEvents } from "./fetch";
import { syncConnectionEvents, tombstoneCalendarConnection } from "./storage";
import {
  connectionKey,
  setCalendarSyncIssue,
  type CalendarSyncIssue,
} from "./sync-errors";

import { enqueueDatabaseWrite } from "~/db/write-queue";
import type { TaskScheduler } from "~/services/task-scheduler";

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

export function scheduleCalendarSync(
  manager: TaskScheduler,
): string | undefined {
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

export async function removeDisconnectedCalendarConnectionFromCallback(
  integrationId: string,
  connectionId: string,
): Promise<void> {
  if (integrationId !== "google-calendar" && integrationId !== "outlook")
    return;

  const generation = calendarSyncGeneration;
  const provider = calendarProviderForIntegration(integrationId);
  const connections = await getProviderConnections();
  const discovered = connections.find((entry) => entry.provider === provider);
  // An omitted provider means discovery failed or authentication is unavailable.
  if (
    generation !== calendarSyncGeneration ||
    !discovered ||
    discovered.connection_ids.includes(connectionId)
  )
    return;

  await removeDisconnectedCalendarConnection(integrationId, connectionId);
}

export function removeDisconnectedCalendarConnection(
  integrationId: string,
  connectionId: string,
): Promise<void> {
  const provider = calendarProviderForIntegration(integrationId);

  if (!provider) return Promise.resolve();

  const key = connectionKey(provider, connectionId);
  calendarSyncGeneration += 1;
  setCalendarSyncIssue(key);
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

  const inventoryFailures = await syncCalendars(
    providerConnections,
    options.signal,
    shouldStop,
  );
  if (shouldStop()) return;

  for (const { provider, connection_ids } of providerConnections) {
    for (const connectionId of connection_ids) {
      if (shouldStop()) return;

      const key = connectionKey(provider, connectionId);
      let issue: CalendarSyncIssue | undefined = inventoryFailures.get(key);
      try {
        const eventIssue = await runForConnection(
          provider,
          connectionId,
          range,
          options,
          generation,
        );
        if (!issue || issue === "sync_failed") issue = eventIssue ?? issue;
      } catch (error) {
        issue ??= "sync_failed";
        console.error(
          `[calendar-sync] Error syncing ${provider} (${connectionId}): ${error}`,
        );
      }
      if (!shouldStop()) setCalendarSyncIssue(key, issue);
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

  const result = await fetchIncomingEvents(ctx);
  if (shouldStop()) return;
  if (result.calendars.length) {
    await enqueueDatabaseWrite("calendar-sync", async () => {
      if (shouldStop()) return;
      await syncConnectionEvents({
        ctx: { ...ctx, calendars: result.calendars },
        incoming: result.events,
        incomingParticipants: result.participants,
      });
    });
  }
  return result.issue;
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

function calendarProviderForIntegration(
  integrationId: string,
): CalendarProviderType | null {
  return integrationId === "google-calendar"
    ? "google"
    : integrationId === "outlook"
      ? "outlook"
      : integrationId === "apple" || integrationId === "apple-calendar"
        ? "apple"
        : null;
}
