import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createCalendarPersister(store: Store) {
  // Load-only: the Rust calendar-sync worker is the sole writer for
  // `calendars.json`. UI toggles go through `useSetCalendarEnabled` →
  // `calendarCommands.setCalendarEnabled` and arrive back here via the
  // file-changed listener. See `plugins/calendar/src/sync_store.rs`.
  return createJsonFilePersister(store, {
    tableName: "calendars",
    filename: "calendars.json",
    label: "CalendarPersister",
  });
}
