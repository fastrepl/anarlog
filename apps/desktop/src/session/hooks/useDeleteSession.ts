import { useCallback } from "react";

import { useIgnoredEvents } from "~/store/tinybase/hooks";
import {
  captureSessionData,
  deleteSessionCascade,
  finalizeSessionDeletion,
} from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useUndoDelete } from "~/store/zustand/undo-delete";

export function useDeleteSession() {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);
  const { ignoreEvent } = useIgnoredEvents();

  return useCallback(
    (sessionId: string, trackingId?: string | null) => {
      if (!store) {
        return;
      }

      if (trackingId) {
        ignoreEvent(trackingId);
      }

      const capturedData = captureSessionData(store, indexes, sessionId);

      invalidateResource("sessions", sessionId);
      void deleteSessionCascade(store, indexes, sessionId, {
        deferFilesystemDelete: true,
      });

      if (capturedData) {
        addDeletion(capturedData, () => {
          void finalizeSessionDeletion(sessionId);
        });
      }
    },
    [store, indexes, ignoreEvent, invalidateResource, addDeletion],
  );
}
