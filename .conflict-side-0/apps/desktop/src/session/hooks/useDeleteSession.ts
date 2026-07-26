import { emitTo, listen } from "@tauri-apps/api/event";
import { getAllWebviewWindows } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { supabase } from "~/auth/client";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import {
  deleteSessionShareBySession,
  ShareManagementError,
} from "~/session-sharing/client";
import { trackPendingSoftDelete } from "~/session/pending-soft-deletes";
import { finalizeSessionDeletion, softDeleteSession } from "~/session/queries";
import {
  loadManagedSharedNoteForSession,
  removeDurableSharedNoteCache,
} from "~/shared-notes/cache";
import { listenerStore } from "~/store/zustand/listener/instance";
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

// Share revocation runs after the local deletion is finalized and must never
// block or fail it — local deletes have to work offline. Auth is read from
// the module-level client because this can run in windows (or the AppRoot
// listener) mounted outside AuthProvider.
async function revokeManagedShare(sessionId: string) {
  if (!supabase) return;
  const { data } = await supabase.auth.getSession();
  const session = data.session;
  if (!session || session.user.is_anonymous === true) return;
  const context = { session, supabase };

  // A failed lookup is not the same as "no share": without a warning a
  // shared link could stay live after delete with no user signal. Retry
  // once (the flush can rethrow an unrelated transient write failure),
  // then surface it.
  const lookupShare = () =>
    loadManagedSharedNoteForSession(session.user.id, sessionId);
  const managedShare = await lookupShare()
    .catch(lookupShare)
    .catch((error: unknown) => {
      console.error("[delete-session] failed to look up managed share", error);
      sonnerToast.warning(
        "Note deleted, but its shared link could not be verified as removed.",
      );
      return null;
    });
  if (!managedShare) return;

  try {
    const deletedShare = await deleteSessionShareBySession(context, {
      workspaceId: managedShare.workspaceId,
      sessionId: managedShare.sessionId,
    });
    if (
      deletedShare.shareId !== null &&
      deletedShare.shareId !== managedShare.shareId
    ) {
      throw new ShareManagementError();
    }
  } catch (error) {
    // Share RPC failures can carry tokens in their messages; log the name only.
    console.error(
      "[delete-session] failed to revoke shared link",
      error instanceof Error ? error.name : typeof error,
    );
    sonnerToast.warning(
      "Note deleted, but its shared link could not be removed.",
    );
    return;
  }

  try {
    await removeDurableSharedNoteCache(session.user.id, managedShare.shareId);
  } catch {
    console.error("[delete-session] failed to clear shared-note cache");
  }
}

function revokeManagedShareBestEffort(sessionId: string): Promise<void> {
  return revokeManagedShare(sessionId).catch((error: unknown) => {
    console.error(
      "[delete-session] failed to revoke shared link",
      error instanceof Error ? error.name : typeof error,
    );
  });
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
      // leaving the note soft-deleted with no undo and no share cleanup.
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
        // removes the session folder or revokes the shared link. It returns
        // its promise so app exit can await the share revocation.
        const finalize = () =>
          commit
            .then(async (deletedData) => {
              if (!deletedData) return;
              await finalizeSessionDeletion(sessionId);
              await revokeManagedShareBestEffort(sessionId);
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
            sonnerToast.error("Could not delete this note. Please try again.");
          } else {
            // The delete committed but main never learned about it, so its
            // finalize-time cleanup will not run. Finalize here — losing the
            // undo window beats leaving the shared link live forever.
            void finalizeSessionDeletion(sessionId);
            void revokeManagedShareBestEffort(sessionId);
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
        await revokeManagedShareBestEffort(payload.sessionId);
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
