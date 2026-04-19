import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createEventPersister(store: Store) {
  // Load-only: the Rust calendar-sync worker is the sole writer for
  // `events.json`. See `plugins/calendar/src/sync_source.rs` and
  // `plugins/calendar/src/sync_store.rs`.
  return createJsonFilePersister(store, {
    tableName: "events",
    filename: "events.json",
    label: "EventPersister",
    jsonFields: {
      participants_json: "participants",
    },
  });
}
