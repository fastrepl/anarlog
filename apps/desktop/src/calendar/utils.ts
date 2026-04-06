import type { Store } from "tinybase/with-schemas";

import type { Schemas } from "~/store/tinybase/store/main";

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

export function findCalendarByTrackingId(
  store: Store<Schemas>,
  {
    provider,
    connectionId,
    trackingId,
  }: {
    provider: string;
    connectionId: string;
    trackingId: string;
  },
): string | null {
  let foundRowId: string | null = null;

  store.forEachRow("calendars", (rowId, _forEachCell) => {
    if (foundRowId) return;
    const row = store.getRow("calendars", rowId);
    if (
      row?.provider === provider &&
      row?.connection_id === connectionId &&
      row?.tracking_id_calendar === trackingId
    ) {
      foundRowId = rowId;
    }
  });

  return foundRowId;
}
