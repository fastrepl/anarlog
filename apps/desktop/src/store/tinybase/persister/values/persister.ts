import { createCustomPersister } from "tinybase/persisters/with-schemas";
import type { Content } from "tinybase/with-schemas";

import type { Schemas } from "@hypr/store";

import type { Store } from "~/store/tinybase/store/main";
import { StoreOrMergeableStore } from "~/store/tinybase/store/shared";
import { commands } from "~/types/tauri.gen";

export function createValuesPersister(store: Store) {
  return createCustomPersister(
    store,
    async () => {
      const result = await commands.getTinybaseValues();
      if (result.status === "error") {
        console.error("[ValuesPersister] load error:", result.error);
        return undefined;
      }

      if (!result.data) {
        return undefined;
      }

      try {
        const values = JSON.parse(result.data) as Record<string, unknown>;
        return [{}, values] as Content<Schemas>;
      } catch (e) {
        console.error("[ValuesPersister] parse error:", e);
        return undefined;
      }
    },
    async () => {
      const values = store.getValues();
      const serialized = JSON.stringify(values);

      const result = await commands.setTinybaseValues(serialized);
      if (result.status === "error") {
        console.error("[ValuesPersister] save error:", result.error);
      }
    },
    () => null,
    () => {},
    (error) => console.error("[ValuesPersister]:", error),
    StoreOrMergeableStore,
  );
}
