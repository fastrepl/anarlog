import * as _UI from "tinybase/ui-react/with-schemas";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { type Schemas } from "@hypr/store";

import { createTaskPersister } from "./persister";

import type { Store } from "~/store/tinybase/store/main";

const { useCreatePersister } = _UI as _UI.WithSchemas<Schemas>;

export function useTaskPersister(store: Store) {
  return useCreatePersister(
    store,
    async (store) => {
      const persister = createTaskPersister(store as Store);
      if (getCurrentWebviewWindowLabel() === "main") {
        await persister.startAutoPersisting();
      } else {
        await persister.startAutoLoad();
      }
      return persister;
    },
    [],
  );
}
