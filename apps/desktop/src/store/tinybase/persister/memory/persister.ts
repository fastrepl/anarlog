import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createMemoryPersister(store: Store) {
  return createJsonFilePersister(store, {
    tableName: "memories",
    filename: "memories.json",
    label: "MemoryPersister",
  });
}
