import { commands as calendarCommands } from "@hypr/plugin-calendar";
import type { CalendarEvent } from "@hypr/plugin-calendar";
import { commands as miscCommands } from "@hypr/plugin-misc";

import type { Ctx } from "../ctx";
import type {
  EventParticipant,
  IncomingEvent,
  IncomingParticipants,
} from "./types";

export class CalendarFetchError extends Error {
  constructor(
    public readonly calendarTrackingId: string,
    public readonly cause: string,
  ) {
    super(
      `Failed to fetch events for calendar ${calendarTrackingId}: ${cause}`,
    );
    this.name = "CalendarFetchError";
  }
}

export async function fetchIncomingEvents(ctx: Ctx): Promise<{
  events: IncomingEvent[];
  participants: IncomingParticipants;
}> {
  const trackingIds = Array.from(ctx.calendarTrackingIdToId.keys());

  const results = await Promise.all(
    trackingIds.map(async (trackingId) => {
      const result = await calendarCommands.listEvents(
        ctx.provider,
        ctx.connectionId,
        {
          calendar_tracking_id: trackingId,
          from: ctx.from.toISOString(),
          to: ctx.to.toISOString(),
        },
      );

      if (result.status === "error") {
        throw new CalendarFetchError(trackingId, result.error);
      }

      return result.data;
    }),
  );

  const calendarEvents = results.flat();
  const events: IncomingEvent[] = [];
  const participants: IncomingParticipants = new Map();

  for (const calendarEvent of calendarEvents) {
    if (
      calendarEvent.attendees.find(
        (attendee) =>
          attendee.is_current_user && attendee.status === "declined",
      )
    ) {
      continue;
    }
    const { event, eventParticipants } =
      await normalizeCalendarEvent(calendarEvent);
    events.push(event);
    if (eventParticipants.length > 0) {
      participants.set(event.tracking_id_event, eventParticipants);
    }
  }

  return { events, participants };
}

async function normalizeCalendarEvent(calendarEvent: CalendarEvent): Promise<{
  event: IncomingEvent;
  eventParticipants: EventParticipant[];
}> {
  const meetingLink =
    calendarEvent.meeting_link ??
    (await extractMeetingLink(
      calendarEvent.description,
      calendarEvent.location,
    ));

  const eventParticipants: EventParticipant[] = [];

  if (calendarEvent.organizer) {
    eventParticipants.push({
      name: calendarEvent.organizer.name ?? undefined,
      email: calendarEvent.organizer.email ?? undefined,
      is_organizer: true,
      is_current_user: calendarEvent.organizer.is_current_user,
    });
  }

  const organizerEmail = calendarEvent.organizer?.email?.toLowerCase();

  for (const attendee of calendarEvent.attendees) {
    if (attendee.role === "nonparticipant") continue;
    if (organizerEmail && attendee.email?.toLowerCase() === organizerEmail)
      continue;
    eventParticipants.push({
      name: attendee.name ?? undefined,
      email: attendee.email ?? undefined,
      is_organizer: false,
      is_current_user: attendee.is_current_user,
    });
  }

  return {
    event: {
      tracking_id_event: calendarEvent.id,
      tracking_id_calendar: calendarEvent.calendar_id,
      title: calendarEvent.title,
      started_at: calendarEvent.started_at,
      ended_at: calendarEvent.ended_at,
      location: calendarEvent.location ?? undefined,
      meeting_link: meetingLink ?? undefined,
      description: calendarEvent.description ?? undefined,
      recurrence_series_id: calendarEvent.recurring_event_id ?? undefined,
      has_recurrence_rules: calendarEvent.has_recurrence_rules,
      is_all_day: calendarEvent.is_all_day,
    },
    eventParticipants,
  };
}

async function extractMeetingLink(
  ...texts: (string | undefined | null)[]
): Promise<string | undefined> {
  for (const text of texts) {
    if (!text) continue;
    const result = await miscCommands.parseMeetingLink(text);
    if (result) return result;
  }
  return undefined;
}
