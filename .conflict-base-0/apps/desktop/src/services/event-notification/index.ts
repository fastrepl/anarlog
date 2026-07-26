import { commands as notificationCommands } from "@hypr/plugin-notification";

import { getIgnoredEventSets } from "~/calendar/ignored-events";
import { liveQueryClient } from "~/db";

export const EVENT_NOTIFICATION_TASK_ID = "eventNotification";
export const EVENT_NOTIFICATION_INTERVAL = 30 * 1000;

const NOTIFY_WINDOW_MS = 5 * 60 * 1000;
const NOTIFIED_EVENTS_TTL_MS = 10 * 60 * 1000;

export type NotifiedEventsMap = Map<string, number>;

type NotificationEventRow = {
  id: string;
  title: string;
  started_at: string;
  tracking_id_event: string;
  recurrence_series_id: string;
};

export async function checkEventNotifications(
  notificationEnabled: boolean,
  notifiedEvents: NotifiedEventsMap,
): Promise<void> {
  if (!notificationEnabled) return;

  const now = Date.now();
  for (const [key, timestamp] of notifiedEvents) {
    if (now - timestamp > NOTIFIED_EVENTS_TTL_MS) notifiedEvents.delete(key);
  }

  const [{ ignoredIds, ignoredSeriesIds }, events] = await Promise.all([
    getIgnoredEventSets(),
    liveQueryClient.execute<NotificationEventRow>(`
      SELECT
        id,
        title,
        started_at,
        tracking_id_event,
        recurrence_series_id
      FROM events
      WHERE deleted_at IS NULL AND started_at <> ''
      ORDER BY started_at, id
    `),
  ]);

  for (const event of events) {
    const startTime = new Date(event.started_at);
    const timeUntilStart = startTime.getTime() - now;
    const notificationKey = `event-${event.id}-${startTime.getTime()}`;

    if (
      event.tracking_id_event &&
      (ignoredIds.has(event.tracking_id_event) ||
        (event.recurrence_series_id &&
          ignoredSeriesIds.has(event.recurrence_series_id)))
    ) {
      continue;
    }

    if (timeUntilStart > 0 && timeUntilStart <= NOTIFY_WINDOW_MS) {
      if (notifiedEvents.has(notificationKey)) continue;
      notifiedEvents.set(notificationKey, now);
      const minutesUntil = Math.ceil(timeUntilStart / 60_000);

      void notificationCommands.showNotification({
        key: notificationKey,
        title: event.title || "Upcoming Event",
        message: `Starting in ${minutesUntil} minute${minutesUntil !== 1 ? "s" : ""}`,
        timeout: null,
        source: { type: "calendar_event", event_id: event.id },
        start_time: Math.floor(startTime.getTime() / 1000),
        participants: null,
        event_details: null,
        action_label: "Open Anarlog",
        action_variant: null,
        options: null,
        footer: null,
        icon: null,
      });
    } else if (timeUntilStart <= 0) {
      notifiedEvents.delete(notificationKey);
    }
  }
}
