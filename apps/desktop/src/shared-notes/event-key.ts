export function sharedNoteEventKey(
  trackingId: string | null | undefined,
  startedAt: string | null | undefined,
): string {
  const id = trackingId?.trim();
  if (!id || !startedAt) return "";
  const start = new Date(startedAt);
  if (Number.isNaN(start.getTime())) return "";
  const key = `${id}|${start.toISOString()}`;
  return key.length <= 512 ? key : "";
}
