import { useEffect } from "react";

import { createSessionRow, type Store, STORE_ID, UI } from "@/store/tinybase";

import { useNotepadSearch } from "./notepad";
import { SessionPanel } from "./session-panel";

const DEFAULT_SESSION_ID = "default";

export function LiveSession() {
  const { searchId } = useNotepadSearch();
  const store = UI.useStore(STORE_ID) as Store | undefined;
  const sessionId = searchId ?? DEFAULT_SESSION_ID;

  useEffect(() => {
    if (!store) return;
    if (!store.hasRow("sessions", sessionId)) {
      createSessionRow(store, sessionId);
      if (searchId) {
        store.setCell("sessions", sessionId, "pipeline_id", searchId);
      }
    }
  }, [store, sessionId, searchId]);

  return <SessionPanel sessionId={sessionId} />;
}
