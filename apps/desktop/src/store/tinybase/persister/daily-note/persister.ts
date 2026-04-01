import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createDailyNotePersister(store: Store) {
  return createJsonFilePersister(store, {
    tableName: "daily_notes",
    filename: "daily_notes.json",
    label: "DailyNotePersister",
  });
}
