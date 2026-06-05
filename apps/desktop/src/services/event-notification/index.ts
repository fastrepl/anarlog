import { commands as notificationCommands } from "@hypr/plugin-notification";

import type * as main from "~/store/tinybase/store/main";
import type * as settings from "~/store/tinybase/store/settings";

export const EVENT_NOTIFICATION_TASK_ID = "eventNotification";
export const EVENT_NOTIFICATION_INTERVAL = 30 * 1000; // 30 sec

const NOTIFY_WINDOW_MS = 5 * 60 * 1000; // 5 minutes before
const NOTIFIED_EVENTS_TTL_MS = 10 * 60 * 1000; // 10 minutes TTL for cleanup

export type NotifiedEventsMap = Map<string, number>;

export function checkEventNotifications(
  store: main.Store,
  settingsStore: settings.Store,
  notifiedEvents: NotifiedEventsMap,
) {
  const notificationEnabled = settingsStore?.getValue("notification_event");
  if (!notificationEnabled || !store) {
    return;
  }

  const now = Date.now();

  for (const [key, timestamp] of notifiedEvents) {
    if (now - timestamp > NOTIFIED_EVENTS_TTL_MS) {
      notifiedEvents.delete(key);
    }
  }

  const ignoredIds = new Set<string>();
  const ignoredSeriesIds = new Set<string>();

  try {
    const raw = store.getValue("ignored_events") as string | undefined;
    if (raw) {
      for (const e of JSON.parse(raw) as Array<{
        tracking_id: string;
      }>) {
        ignoredIds.add(e.tracking_id);
      }
    }
  } catch {}

  try {
    const raw = store.getValue("ignored_recurring_series") as
      | string
      | undefined;
    if (raw) {
      for (const e of JSON.parse(raw) as Array<{ id: string }>) {
        ignoredSeriesIds.add(e.id);
      }
    }
  } catch {}

  store.forEachRow("events", (eventId, _forEachCell) => {
    const event = store.getRow("events", eventId);
    if (!event?.started_at) return;

    const startTime = new Date(String(event.started_at));
    const timeUntilStart = startTime.getTime() - now;
    const notificationKey = `event-${eventId}-${startTime.getTime()}`;

    const trackingId = event.tracking_id_event as string | undefined;
    const recurrenceSeriesId = event.recurrence_series_id as string | undefined;

    if (trackingId) {
      if (ignoredIds.has(trackingId)) return;
      if (recurrenceSeriesId && ignoredSeriesIds.has(recurrenceSeriesId))
        return;
    }

    if (timeUntilStart > 0 && timeUntilStart <= NOTIFY_WINDOW_MS) {
      if (notifiedEvents.has(notificationKey)) {
        return;
      }

      notifiedEvents.set(notificationKey, now);

      const title = String(event.title || "Upcoming Event");
      const minutesUntil = Math.ceil(timeUntilStart / 60000);

      void notificationCommands.showNotification({
        key: notificationKey,
        title: title,
        message: `Starting in ${minutesUntil} minute${minutesUntil !== 1 ? "s" : ""}`,
        timeout: null,
        source: { type: "calendar_event", event_id: eventId },
        start_time: Math.floor(startTime.getTime() / 1000),
        participants: null,
        event_details: null,
        action_label: "Open notes",
        action_variant: null,
        options: null,
        footer: null,
        icon: null,
      });
    } else if (timeUntilStart <= 0) {
      notifiedEvents.delete(notificationKey);
    }
  });
}
