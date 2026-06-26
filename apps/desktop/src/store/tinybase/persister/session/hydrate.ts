import type { Store } from "tinybase/with-schemas";

import type { Schemas } from "@hypr/store";
import { asTablesChanges } from "@hypr/tinybase-utils";

import { loadSingleSession } from "./load";
import type { LoadedSessionData } from "./load";

import { getDataDir } from "~/store/tinybase/persister/shared/paths";

function hasLoadedRows(data: LoadedSessionData): boolean {
  return Object.values(data).some((table) => Object.keys(table).length > 0);
}

export async function hydrateSessionContent(
  store: Store<Schemas>,
  sessionId: string,
): Promise<boolean> {
  const dataDir = await getDataDir();
  const loadResult = await loadSingleSession(dataDir, sessionId);

  if (loadResult.status === "error") {
    console.error(
      `[SessionPersister] hydrate error for ${sessionId}:`,
      loadResult.error,
    );
    return false;
  }

  if (!hasLoadedRows(loadResult.data)) {
    return true;
  }

  store.applyChanges(asTablesChanges(loadResult.data));
  return true;
}
