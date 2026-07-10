import { useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { useInitializeStore } from "./initialize";
import { isLegacyDataPersistenceDisabled } from "./legacy-persistence";
import { type Store } from "./main";
import { registerSaveHandler } from "./save";

import { useHumanPersister } from "~/store/tinybase/persister/human";
import { useOrganizationPersister } from "~/store/tinybase/persister/organization";
import { useSessionPersister } from "~/store/tinybase/persister/session";
import { useValuesPersister } from "~/store/tinybase/persister/values";

export function useMainPersisters(store: Store) {
  const valuesPersister = useValuesPersister(store);
  const sessionPersister = useSessionPersister(store);
  const organizationPersister = useOrganizationPersister(store);
  const humanPersister = useHumanPersister(store);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    const persisters = [
      { id: "values", persister: valuesPersister },
      { id: "session", persister: sessionPersister },
      { id: "organization", persister: organizationPersister },
      { id: "human", persister: humanPersister },
    ];

    const unsubscribes = persisters
      .filter(({ persister }) => persister)
      .map(({ id, persister }) =>
        registerSaveHandler(id, async () => {
          if (id !== "values" && isLegacyDataPersistenceDisabled()) {
            return;
          }
          await persister!.save();
        }),
      );

    return () => {
      unsubscribes.forEach((unsub) => unsub());
    };
  }, [
    valuesPersister,
    sessionPersister,
    organizationPersister,
    humanPersister,
  ]);

  useInitializeStore(store, {
    session: sessionPersister,
    human: humanPersister,
    values: valuesPersister,
  });

  return {
    valuesPersister,
    sessionPersister,
    organizationPersister,
    humanPersister,
  };
}
