import { t } from "@lingui/core/macro";

import { commands as notificationCommands } from "@anlg/plugin-notification";

import { getIgnoredEventSets } from "~/calendar/ignored-events";
import { parseEventParticipants } from "~/calendar/queries";
import { liveQueryClient } from "~/db";
import { compactBriefText } from "~/session/insights/pre-meeting";

export const EVENT_NOTIFICATION_TASK_ID = "eventNotification";
export const EVENT_NOTIFICATION_INTERVAL = 30 * 1000;

const NOTIFY_WINDOW_MS = 5 * 60 * 1000;
const NOTIFICATION_LINGER_MS = 5 * 60 * 1000;
const NOTIFIED_EVENTS_TTL_MS = 10 * 60 * 1000;
const MAX_NOTIFICATION_CONTEXT_LENGTH = 140;

export type NotifiedEventsMap = Map<string, number>;

type NotificationEventRow = {
  id: string;
  title: string;
  started_at: string;
  tracking_id_event: string;
  recurrence_series_id: string;
  is_all_day: boolean | number;
  location: string;
  description: string;
  participants_json: string;
};

type PriorMeetingSummaryRow = { body: string };

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
        recurrence_series_id,
        is_all_day,
        location,
        description,
        participants_json
      FROM events
      WHERE deleted_at IS NULL AND started_at <> '' AND is_all_day = 0
      ORDER BY started_at, id
    `),
  ]);

  for (const event of events) {
    if (Boolean(event.is_all_day)) {
      continue;
    }

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
      let previousSummary = "";
      try {
        previousSummary = await loadPriorMeetingSummary(event);
      } catch (error) {
        console.error(
          "[event-notification] failed to load pre-meeting context",
          error,
        );
      }
      const participantNames = [
        ...new Set(
          parseEventParticipants(event.participants_json)
            .filter((participant) => participant.is_current_user !== true)
            .map(
              (participant) =>
                participant.name?.trim() || participant.email?.trim() || "",
            )
            .filter(Boolean),
        ),
      ];

      void notificationCommands.showNotification({
        key: notificationKey,
        title: event.title || t`Upcoming Event`,
        message: buildPreMeetingNotificationMessage({
          minutesUntil,
          previousSummary,
          description: event.description,
          location: event.location,
          participantNames,
        }),
        timeout: {
          secs: Math.ceil((timeUntilStart + NOTIFICATION_LINGER_MS) / 1000),
          nanos: 0,
        },
        source: { type: "calendar_event", event_id: event.id },
        start_time: null,
        participants: null,
        event_details: null,
        action_label: t`Open Anarlog`,
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

export function buildPreMeetingNotificationMessage({
  minutesUntil,
  previousSummary,
  description,
  location,
  participantNames,
}: {
  minutesUntil: number;
  previousSummary?: string;
  description?: string;
  location?: string;
  participantNames?: string[];
}): string {
  const timing =
    minutesUntil === 1
      ? t`Starting in 1 minute`
      : t`Starting in ${minutesUntil} minutes`;
  const summary = compactBriefText(
    previousSummary ?? "",
    MAX_NOTIFICATION_CONTEXT_LENGTH,
  );
  if (summary) {
    return `${timing} · ${t`Last time: ${summary}`}`;
  }

  const eventContext = compactBriefText(
    description ?? "",
    MAX_NOTIFICATION_CONTEXT_LENGTH,
  );
  if (eventContext) {
    return `${timing} · ${eventContext}`;
  }

  const names = [...new Set(participantNames ?? [])]
    .map((name) => name.trim())
    .filter(Boolean)
    .slice(0, 3);
  if (names.length > 0) {
    return `${timing} · ${t`With ${names.join(", ")}`}`;
  }

  const eventLocation = compactBriefText(
    location ?? "",
    MAX_NOTIFICATION_CONTEXT_LENGTH,
  );
  return eventLocation ? `${timing} · ${eventLocation}` : timing;
}

async function loadPriorMeetingSummary(
  event: NotificationEventRow,
): Promise<string> {
  if (!event.recurrence_series_id.trim()) {
    return "";
  }

  const rows = await liveQueryClient.execute<PriorMeetingSummaryRow>(
    `
      SELECT document.body
      FROM sessions AS session
      JOIN session_documents AS document
        ON document.session_id = session.id
        AND document.kind IN ('summary', 'template_output')
        AND document.deleted_at IS NULL
      WHERE session.series_id = ?
        AND session.deleted_at IS NULL
        AND session.locked = 0
        AND COALESCE(NULLIF(session.started_at, ''), session.created_at) < ?
      ORDER BY
        COALESCE(NULLIF(session.started_at, ''), session.created_at) DESC,
        document.sort_order,
        document.created_at,
        document.id
      LIMIT 1
    `,
    [event.recurrence_series_id, event.started_at],
  );

  return rows[0]?.body ?? "";
}
