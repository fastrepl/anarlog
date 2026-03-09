import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createCalendarPersister(store: Store) {
  return createJsonFilePersister(store, {
    tableName: "calendars",
    filename: "calendars.json",
    label: "CalendarPersister",
  });
}
