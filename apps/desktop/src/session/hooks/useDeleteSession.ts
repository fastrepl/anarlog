import type { Session, SupabaseClient } from "@supabase/supabase-js";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getAllWebviewWindows } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { useOptionalAuth } from "~/auth";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import {
  deleteSessionShareBySession,
  ShareManagementError,
} from "~/session-sharing/client";
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
// block or fail it — local deletes have to work offline.
async function revokeManagedShare(
  context: { session: Session; supabase: SupabaseClient },
  sessionId: string,
) {
  const managedShare = await loadManagedSharedNoteForSession(
    context.session.user.id,
    sessionId,
  ).catch((error: unknown) => {
    console.error("[delete-session] failed to look up managed share", error);
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
    await removeDurableSharedNoteCache(
      context.session.user.id,
      managedShare.shareId,
    );
  } catch {
    console.error("[delete-session] failed to clear shared-note cache");
  }
}

function revokeManagedShareBestEffort(
  auth: {
    session: Session | null | undefined;
    supabase: SupabaseClient | null;
  } | null,
  sessionId: string,
) {
  if (
    !auth?.session ||
    !auth.supabase ||
    auth.session.user.is_anonymous === true
  ) {
    return;
  }
  void revokeManagedShare(
    { session: auth.session, supabase: auth.supabase },
    sessionId,
  );
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
  const auth = useOptionalAuth();
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);
  const { ignoreEvent } = useIgnoredEvents();

  return useCallback(
    (sessionId: string, trackingId?: string | null, batchId?: string) => {
      const windowLabel = getCurrentWebviewWindowLabel();
      const listenerState = listenerStore.getState();
      const live = listenerState.live;

      if (
        live.sessionId === sessionId &&
        (live.status === "active" || live.loading)
      ) {
        listenerState.stop();
      }

      void (async () => {
        let didDelete = false;
        try {
          const deletedData = await softDeleteSession(sessionId);
          if (!deletedData) return;
          didDelete = true;

          if (trackingId) ignoreEvent(trackingId);
          invalidateResource("sessions", sessionId);
          if (windowLabel === "main") {
            const finalize = () => {
              void finalizeSessionDeletion(sessionId);
              revokeManagedShareBestEffort(auth, sessionId);
            };
            if (batchId) {
              addDeletion(deletedData, finalize, batchId);
            } else {
              addDeletion(deletedData, finalize);
            }
          } else {
            await emitTo("main", SESSION_DELETED_FOR_UNDO_EVENT, {
              sessionId,
              data: deletedData,
            } satisfies SessionDeletedForUndoPayload);
          }
        } catch (error) {
          console.error("[delete-session] failed to finish deletion", error);
          sonnerToast.error("Could not delete this note. Please try again.");
        } finally {
          if (didDelete) {
            await closeSessionNoteWindows(sessionId);
          }
        }
      })();
    },
    [
      auth?.session,
      auth?.supabase,
      ignoreEvent,
      invalidateResource,
      addDeletion,
    ],
  );
}

export function useRemoteSessionDeletionUndoListener(active: boolean) {
  const auth = useOptionalAuth();
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
      addDeletion(payload.data, () => {
        void finalizeSessionDeletion(payload.sessionId);
        revokeManagedShareBestEffort(auth, payload.sessionId);
      });
      void closeSessionNoteWindows(payload.sessionId);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [active, auth, invalidateResource, addDeletion]);
}
