import {
  type EventDetails,
  commands as notificationCommands,
  type Participant,
} from "@hypr/plugin-notification";

import { getAllEvents } from "~/calendar/queries";
import { findSessionByEventId } from "~/session/utils";
import type * as main from "~/store/tinybase/store/main";
import type * as settings from "~/store/tinybase/store/settings";

export const EVENT_NOTIFICATION_TASK_ID = "eventNotification";
export const EVENT_NOTIFICATION_INTERVAL = 30 * 1000; // 30 sec

const NOTIFY_WINDOW_MS = 5 * 60 * 1000; // 5 minutes before
const NOTIFIED_EVENTS_TTL_MS = 10 * 60 * 1000; // 10 minutes TTL for cleanup

export type NotifiedEventsMap = Map<string, number>;

function getParticipantsForSession(
  store: main.Store,
  sessionId: string,
): Participant[] {
  const participants: Participant[] = [];

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mapping = store.getRow("mapping_session_participant", mappingId);
    if (mapping?.session_id !== sessionId) return;

    const humanId = mapping.human_id as string | undefined;
    if (!humanId) return;

    const human = store.getRow("humans", humanId);
    if (!human) return;

    participants.push({
      name: (human.name as string) || null,
      email: (human.email as string) || "",
      status: "Accepted",
    });
  });

  return participants;
}

export async function checkEventNotifications(
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

  const allEvents = await getAllEvents();

  for (const event of allEvents) {
    if (!event.startedAt) continue;

    const startTime = new Date(event.startedAt);
    const timeUntilStart = startTime.getTime() - now;
    const notificationKey = `event-${event.id}-${startTime.getTime()}`;

    const trackingId = event.trackingIdEvent;
    const recurrenceSeriesId = event.recurrenceSeriesId;

    if (trackingId) {
      if (ignoredIds.has(trackingId)) continue;
      if (recurrenceSeriesId && ignoredSeriesIds.has(recurrenceSeriesId))
        continue;
    }

    if (timeUntilStart > 0 && timeUntilStart <= NOTIFY_WINDOW_MS) {
      if (notifiedEvents.has(notificationKey)) {
        continue;
      }

      notifiedEvents.set(notificationKey, now);

      const title = event.title || "Upcoming Event";
      const minutesUntil = Math.ceil(timeUntilStart / 60000);

      const eventDetails: EventDetails = {
        what: title,
        timezone: null,
        location: event.meetingLink || event.location || null,
      };

      let participants: Participant[] | null = null;
      const sessionId = await findSessionByEventId(store, event.id);
      if (sessionId) {
        const sessionParticipants = getParticipantsForSession(store, sessionId);
        if (sessionParticipants.length > 0) {
          participants = sessionParticipants;
        }
      }

      void notificationCommands.showNotification({
        key: notificationKey,
        title: title,
        message: `Starting in ${minutesUntil} minute${minutesUntil !== 1 ? "s" : ""}`,
        timeout: null,
        source: { type: "calendar_event", event_id: event.id },
        start_time: Math.floor(startTime.getTime() / 1000),
        participants: participants,
        event_details: eventDetails,
        action_label: "Start listening",
        options: null,
        footer: null,
        icon: null,
      });
    } else if (timeUntilStart <= 0) {
      notifiedEvents.delete(notificationKey);
    }
  }
}
