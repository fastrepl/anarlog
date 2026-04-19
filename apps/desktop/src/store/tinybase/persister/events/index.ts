import * as _UI from "tinybase/ui-react/with-schemas";

import { type Schemas } from "@hypr/store";

import { createEventPersister } from "./persister";

import type { Store } from "~/store/tinybase/store/main";

const { useCreatePersister } = _UI as _UI.WithSchemas<Schemas>;

export function useEventsPersister(store: Store) {
  return useCreatePersister(
    store,
    async (store) => {
      const persister = createEventPersister(store as Store);
      await persister.startAutoLoad();
      return persister;
    },
    [],
  );
}
