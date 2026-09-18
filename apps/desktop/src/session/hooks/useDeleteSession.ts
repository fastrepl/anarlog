import { t } from "@lingui/core/macro";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getAllWebviewWindows } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@anlg/plugin-windows";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useIgnoredEvents } from "~/calendar/ignored-events";
import { deleteCloudApiSnapshotBestEffort } from "~/cloud-api/client";
import { trackPendingSoftDelete } from "~/session/pending-soft-deletes";
import { finalizeSessionDeletion, softDeleteSession } from "~/session/queries";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import {
  type DeletedSessionData,
  useUndoDelete,
} from "~/store/zustand/undo-delete";

const SESSION_DELETED_FOR_UNDO_EVENT = "anlg://session-deleted-for-undo";

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
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);
  const clearDeletion = useUndoDelete((state) => state.clearDeletion);
  const { ignoreEvent, unignoreEvent, isIgnored } = useIgnoredEvents();

  return useCallback(
    (
      sessionId: string,
      options?: {
        trackingId?: string | null;
        batchId?: string;
        title?: string;
      },
    ) => {
      const { trackingId, batchId, title } = options ?? {};
      // A repeat delete would replace the pending tombstone and its finalize
      // callback, then no-op in softDeleteSession and clear the undo toast —
      // leaving the note soft-deleted with no undo and no cleanup.
      if (useUndoDelete.getState().pendingDeletions[sessionId]) {
        return;
      }
      const windowLabel = getCurrentWebviewWindowLabel();
      const isMainWindow = windowLabel === "main";
      const listenerState = listenerStore.getState();
      const live = listenerState.live;

      if (
        live.sessionId === sessionId &&
        (live.status === "active" || live.loading)
      ) {
        listenerState.stop();
      }

      // Optimistic path: hide the row, drop tab history, and show the undo
      // toast before the soft-delete commits; rolled back below on failure.
      const tombstone = new Date().toISOString();
      const wasIgnored = trackingId ? isIgnored(trackingId, null) : false;
      const hadOpenTab = useTabs
        .getState()
        .tabs.some((tab) => tab.type === "sessions" && tab.id === sessionId);
      if (trackingId) ignoreEvent(trackingId);
      invalidateResource("sessions", sessionId);

      const commit = softDeleteSession(sessionId, tombstone);
      trackPendingSoftDelete(sessionId, commit);

      const clearOptimisticDeletion = () => {
        const pending = useUndoDelete.getState().pendingDeletions[sessionId];
        if (pending?.data.tombstone === tombstone) {
          clearDeletion(sessionId);
        }
      };

      if (isMainWindow) {
        // Finalize gates on the commit so a failed or no-op delete never
        // removes the session folder.
        const finalize = () =>
          commit
            .then(async (deletedData) => {
              if (!deletedData) return;
              await finalizeSessionDeletion(sessionId);
              deleteCloudApiSnapshotBestEffort(sessionId);
            })
            .catch(() => undefined);
        addDeletion(
          {
            session: { id: sessionId, title: title ?? "" },
            tombstone,
            deletedAt: Date.now(),
          },
          finalize,
          batchId,
        );
      }

      void (async () => {
        let didDelete = false;
        try {
          const deletedData = await commit;
          if (!deletedData) {
            // The session was already deleted; drop the optimistic toast.
            if (isMainWindow) clearOptimisticDeletion();
            return;
          }
          didDelete = true;

          if (!isMainWindow) {
            await emitTo("main", SESSION_DELETED_FOR_UNDO_EVENT, {
              sessionId,
              data: deletedData,
            } satisfies SessionDeletedForUndoPayload);
          }
        } catch (error) {
          console.error("[delete-session] failed to finish deletion", error);
          if (!didDelete) {
            if (isMainWindow) clearOptimisticDeletion();
            // Only undo the optimistic ignore; a pre-existing ignore must
            // survive a failed delete.
            if (trackingId && !wasIgnored) unignoreEvent(trackingId);
            if (hadOpenTab) {
              useTabs
                .getState()
                .openCurrent({ type: "sessions", id: sessionId });
            }
            sonnerToast.error(t`Could not delete this note. Please try again.`);
          } else {
            // The delete committed but main never learned about it, so its
            // finalize-time cleanup will not run. Finalize here.
            void finalizeSessionDeletion(sessionId);
            deleteCloudApiSnapshotBestEffort(sessionId);
          }
        } finally {
          if (didDelete) {
            await closeSessionNoteWindows(sessionId);
          }
        }
      })();
    },
    [
      ignoreEvent,
      unignoreEvent,
      isIgnored,
      invalidateResource,
      addDeletion,
      clearDeletion,
    ],
  );
}

export function useRemoteSessionDeletionUndoListener(active: boolean) {
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);

  useEffect(() => {
    if (!active) {
      return;
    }

    let unlisten: (() => void) | undefined;

    void listen(SESSION_DELETED_FOR_UNDO_EVENT, (event) => {
      const payload = event.payload;
      if (!isSessionDeletedForUndoPayload(payload)) {
        return;
      }

      invalidateResource("sessions", payload.sessionId);
      addDeletion(payload.data, async () => {
        await finalizeSessionDeletion(payload.sessionId);
        deleteCloudApiSnapshotBestEffort(payload.sessionId);
      });
      void closeSessionNoteWindows(payload.sessionId);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [active, invalidateResource, addDeletion]);
}
