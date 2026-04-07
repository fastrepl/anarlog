import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createTaskPersister(store: Store) {
  return createJsonFilePersister(store, {
    tableName: "tasks",
    filename: "tasks.json",
    label: "TaskPersister",
    jsonFields: {
      body_json: "body",
    },
  });
}
