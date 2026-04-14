import type { Ctx } from "../ctx";
import type { ExistingEvent } from "./types";

import { getEventsByCalendarIds } from "~/calendar/queries";

export async function fetchExistingEvents(ctx: Ctx): Promise<ExistingEvent[]> {
  const calendarIds = Array.from(ctx.calendarIds);
  if (calendarIds.length === 0) return [];

  const records = await getEventsByCalendarIds(
    calendarIds,
    ctx.from.toISOString(),
    ctx.to.toISOString(),
  );

  return records.map((r) => ({
    id: r.id,
    tracking_id_event: r.trackingIdEvent,
    calendar_id: r.calendarId,
    title: r.title,
    started_at: r.startedAt,
    ended_at: r.endedAt,
    location: r.location,
    meeting_link: r.meetingLink,
    description: r.description,
    note: r.note,
    recurrence_series_id: r.recurrenceSeriesId,
    has_recurrence_rules: r.hasRecurrenceRules,
    provider: r.provider,
    user_id: ctx.userId,
    created_at: r.createdAt,
  }));
}
