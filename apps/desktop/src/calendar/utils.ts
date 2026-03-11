import type { Store } from "tinybase/with-schemas";

import type { Schemas } from "~/store/tinybase/store/main";

export function findCalendarByTrackingId(
  store: Store<Schemas>,
  trackingId: string,
): string | null {
  let foundRowId: string | null = null;

  store.forEachRow("calendars", (rowId, _forEachCell) => {
    if (foundRowId) return;
    const row = store.getRow("calendars", rowId);
    if (row?.tracking_id_calendar === trackingId) {
      foundRowId = rowId;
    }
  });

  return foundRowId;
}
