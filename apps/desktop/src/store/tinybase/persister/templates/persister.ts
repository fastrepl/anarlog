import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createTemplatePersister(store: Store) {
  return createJsonFilePersister(store, {
    tableName: "templates",
    filename: "templates.json",
    label: "TemplatePersister",
  });
}
