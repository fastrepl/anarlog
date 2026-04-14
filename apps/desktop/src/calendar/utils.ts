import { getAllCalendars } from "~/calendar/queries";

export function getCalendarTrackingKey({
  provider,
  connectionId,
  trackingId,
}: {
  provider: string | undefined;
  connectionId: string | undefined;
  trackingId: string | undefined;
}) {
  return [provider ?? "", connectionId ?? "", trackingId ?? ""].join(":");
}

export async function findCalendarByTrackingId({
  provider,
  connectionId,
  trackingId,
}: {
  provider: string;
  connectionId: string;
  trackingId: string;
}): Promise<string | null> {
  const calendars = await getAllCalendars();

  for (const cal of calendars) {
    if (
      cal.provider === provider &&
      cal.connectionId === connectionId &&
      cal.trackingIdCalendar === trackingId
    ) {
      return cal.id;
    }
  }

  return null;
}
