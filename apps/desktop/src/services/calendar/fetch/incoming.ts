import { commands as authCommands } from "@hypr/plugin-auth";
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
  const currentUser = await getCurrentUserInfo();

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
    const { event, eventParticipants } = await normalizeCalendarEvent(
      calendarEvent,
      currentUser,
    );
    events.push(event);
    if (eventParticipants.length > 0) {
      participants.set(event.tracking_id_event, eventParticipants);
    }
  }

  return { events, participants };
}

export async function normalizeCalendarEvent(
  calendarEvent: CalendarEvent,
  currentUser?: CurrentUserInfo | null,
): Promise<{
  event: IncomingEvent;
  eventParticipants: EventParticipant[];
}> {
  const meetingLink =
    calendarEvent.meeting_link ??
    (await extractMeetingLink(
      calendarEvent.description,
      calendarEvent.location,
    ));

  const rawParticipants: EventParticipant[] = [];
  const currentUserEmail = currentUser?.email?.trim().toLowerCase();

  if (calendarEvent.organizer) {
    rawParticipants.push({
      name: calendarEvent.organizer.name ?? undefined,
      email: calendarEvent.organizer.email ?? undefined,
      is_organizer: true,
      is_current_user: isCurrentUserParticipant(
        calendarEvent.organizer.email,
        currentUserEmail,
        calendarEvent.organizer.is_current_user,
      ),
    });
  }

  for (const attendee of calendarEvent.attendees) {
    if (attendee.role === "nonparticipant") continue;
    rawParticipants.push({
      name: attendee.name ?? undefined,
      email: attendee.email ?? undefined,
      is_organizer: false,
      is_current_user: isCurrentUserParticipant(
        attendee.email,
        currentUserEmail,
        attendee.is_current_user,
      ),
    });
  }

  if (shouldInjectCurrentUser(rawParticipants, currentUser)) {
    rawParticipants.unshift({
      name: currentUser.name,
      email: currentUser.email,
      is_organizer: false,
      is_current_user: true,
    });
  }

  const eventParticipants = dedupeEventParticipants(rawParticipants);

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

function dedupeEventParticipants(
  participants: EventParticipant[],
): EventParticipant[] {
  const deduped: EventParticipant[] = [];
  const keyedIndexes = new Map<string, number>();

  for (const participant of participants) {
    const key = getParticipantKey(participant);
    if (!key) {
      deduped.push(participant);
      continue;
    }

    const existingIndex = keyedIndexes.get(key);
    if (existingIndex === undefined) {
      keyedIndexes.set(key, deduped.length);
      deduped.push(participant);
      continue;
    }

    deduped[existingIndex] = mergeParticipants(
      deduped[existingIndex],
      participant,
    );
  }

  return deduped;
}

type CurrentUserInfo = {
  email: string;
  name?: string;
};

async function getCurrentUserInfo(): Promise<CurrentUserInfo | null> {
  const result = await authCommands.getAccountInfo();
  if (result.status !== "ok") {
    return null;
  }

  const email = result.data?.email?.trim();
  if (!email) {
    return null;
  }

  const name = result.data?.fullName?.trim();
  return {
    email,
    name: name || undefined,
  };
}

function isCurrentUserParticipant(
  email: string | null | undefined,
  currentUserEmail: string | undefined,
  providerFlag: boolean,
): boolean {
  if (providerFlag) {
    return true;
  }

  if (!currentUserEmail) {
    return false;
  }

  return email?.trim().toLowerCase() === currentUserEmail;
}

function shouldInjectCurrentUser(
  participants: EventParticipant[],
  currentUser?: CurrentUserInfo | null,
): currentUser is CurrentUserInfo {
  const currentUserEmail = currentUser?.email?.trim().toLowerCase();
  if (!currentUserEmail) {
    return false;
  }

  return !participants.some(
    (participant) =>
      participant.is_current_user ||
      participant.email?.trim().toLowerCase() === currentUserEmail,
  );
}

function getParticipantKey(participant: EventParticipant): string | null {
  const email = participant.email?.trim().toLowerCase();
  if (email) {
    return `email:${email}`;
  }

  if (participant.is_current_user) {
    return "current-user";
  }

  return null;
}

function mergeParticipants(
  existing: EventParticipant,
  incoming: EventParticipant,
): EventParticipant {
  return {
    name: pickPreferredText(existing.name, incoming.name),
    email: pickPreferredText(existing.email, incoming.email),
    is_organizer:
      existing.is_organizer || incoming.is_organizer ? true : undefined,
    is_current_user:
      existing.is_current_user || incoming.is_current_user ? true : undefined,
  };
}

function pickPreferredText(
  existing?: string,
  incoming?: string,
): string | undefined {
  const existingText = existing?.trim();
  const incomingText = incoming?.trim();

  if (!existingText) {
    return incomingText || undefined;
  }

  if (!incomingText) {
    return existingText;
  }

  return incomingText.length > existingText.length
    ? incomingText
    : existingText;
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
