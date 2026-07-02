import { emitTo, listen } from "@tauri-apps/api/event";
import { getAllWebviewWindows } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { useIgnoredEvents } from "~/store/tinybase/hooks";
import {
  captureSessionData,
  deleteSessionCascade,
  finalizeSessionDeletion,
} from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import {
  type DeletedSessionData,
  useUndoDelete,
} from "~/store/zustand/undo-delete";

const SESSION_DELETED_FOR_UNDO_EVENT = "hypr://session-deleted-for-undo";

type SessionDeletedForUndoPayload = {
  sessionId: string;
  data: DeletedSessionData;
};

async function closeSessionNoteWindows(sessionId: string) {
  try {
    const noteWindowLabel = `note-${sessionId}`;
    const windows = await getAllWebviewWindows();
    await Promise.all(
      windows
        .filter((window) => window.label === noteWindowLabel)
        .map((window) => window.close().catch(() => undefined)),
    );
  } catch {
    // Closing note windows should not block the deletion path.
  }
}

function isSessionDeletedForUndoPayload(
  payload: unknown,
): payload is SessionDeletedForUndoPayload {
  return (
    typeof payload === "object" &&
    payload !== null &&
    "sessionId" in payload &&
    typeof payload.sessionId === "string" &&
    "data" in payload &&
    typeof payload.data === "object" &&
    payload.data !== null
  );
}

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
      const windowLabel = getCurrentWebviewWindowLabel();

      invalidateResource("sessions", sessionId);
      deleteSessionCascade(store, indexes, sessionId, {
        deferFilesystemDelete: true,
      });

      void (async () => {
        try {
          if (capturedData) {
            if (windowLabel === "main") {
              addDeletion(capturedData, () => {
                void finalizeSessionDeletion(sessionId);
              });
            } else {
              await emitTo("main", SESSION_DELETED_FOR_UNDO_EVENT, {
                sessionId,
                data: capturedData,
              } satisfies SessionDeletedForUndoPayload);
            }
          }
        } catch {
          // The note was already deleted locally, so still close matching windows.
        } finally {
          await closeSessionNoteWindows(sessionId);
        }
      })();
    },
    [store, indexes, ignoreEvent, invalidateResource, addDeletion],
  );
}

export function useRemoteSessionDeletionUndoListener(active: boolean) {
  const store = main.UI.useStore(main.STORE_ID);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);

  useEffect(() => {
    if (!active || !store) {
      return;
    }

    let unlisten: (() => void) | undefined;

    void listen(SESSION_DELETED_FOR_UNDO_EVENT, (event) => {
      const payload = event.payload;
      if (!isSessionDeletedForUndoPayload(payload)) {
        return;
      }

      invalidateResource("sessions", payload.sessionId);
      deleteSessionCascade(store, indexes, payload.sessionId, {
        deferFilesystemDelete: true,
      });
      addDeletion(payload.data, () => {
        void finalizeSessionDeletion(payload.sessionId);
      });
      void closeSessionNoteWindows(payload.sessionId);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [active, store, indexes, invalidateResource, addDeletion]);
}
