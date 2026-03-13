import * as _UI from "tinybase/ui-react/with-schemas";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { type Schemas } from "@hypr/store";

import { createPromptPersister } from "./persister";

import type { Store } from "~/store/tinybase/store/main";

const { useCreatePersister } = _UI as _UI.WithSchemas<Schemas>;

export function usePromptPersister(store: Store) {
  return useCreatePersister(
    store,
    async (store) => {
      const persister = createPromptPersister(store as Store);
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
