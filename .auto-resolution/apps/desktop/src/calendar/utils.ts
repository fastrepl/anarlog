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
