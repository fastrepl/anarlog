export const TIMELINE_BLOCK_MS = 30 * 60 * 1000;

export type MeetingTimelineEntry = {
  id: string;
  type: "session" | "event";
  title: string;
  calendarId: string | null;
  trackingId?: string | null;
  recurrenceSeriesId?: string | null;
  start: Date;
  end: Date | null;
  selected: boolean;
  muted: boolean;
};

export function normalizeEndMs(start: Date, end: Date | null): number {
  const startMs = start.getTime();
  const endMs = end?.getTime();

  if (!endMs || endMs <= startMs) {
    return startMs + TIMELINE_BLOCK_MS;
  }

  return endMs;
}
