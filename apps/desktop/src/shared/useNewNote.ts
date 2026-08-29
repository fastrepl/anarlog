import { t } from "@lingui/core/macro";
import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import { useShallow } from "zustand/shallow";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { createSession } from "~/session/queries";
import { folderIdForNewNote, useSidebarNotes } from "~/sidebar/note-filter";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { reservePendingUpload } from "~/stt/pending-upload";

function createNoteSession() {
  const { noteFilter, folderFilter } = useSidebarNotes.getState();
  const folderId = folderIdForNewNote(noteFilter, folderFilter);
  return folderId === undefined
    ? createSession()
    : createSession("", undefined, { folder_id: folderId });
}

export function useNewNote({
  behavior = "new",
}: {
  behavior?: "new" | "current";
} = {}) {
  const { openNew, openCurrent } = useTabs(
    useShallow((state) => ({
      openNew: state.openNew,
      openCurrent: state.openCurrent,
    })),
  );

  const handler = useCallback(() => {
    const ff = behavior === "new" ? openNew : openCurrent;
    void createNoteSession()
      .then((sessionId) => {
        ff({ type: "sessions", id: sessionId });
      })
      .catch((error) => {
        console.error("[session] failed to create note", error);
      });
  }, [openNew, openCurrent, behavior]);

  return handler;
}

export function useNewNoteAndListen({
  behavior = "new",
}: {
  behavior?: "new" | "current";
} = {}) {
  const handler = useCallback(
    () => openNewNoteAndListen({ behavior }),
    [behavior],
  );

  return handler;
}

export function openNewNoteAndListen({
  behavior = "new",
}: {
  behavior?: "new" | "current";
} = {}) {
  const { status, sessionId: liveSessionId } = listenerStore.getState().live;

  if (status === "active" && liveSessionId) {
    const { openNew, openCurrent } = useTabs.getState();
    const open = behavior === "new" ? openNew : openCurrent;
    open({ type: "sessions", id: liveSessionId });
    return;
  }

  void createNoteSession()
    .then((sessionId) => {
      openSessionAndListen(sessionId, { behavior });
    })
    .catch((error) => {
      console.error("[session] failed to create listening note", error);
    });
}

export function openSessionAndListen(
  sessionId: string,
  {
    behavior = "new",
  }: {
    behavior?: "new" | "current";
  } = {},
) {
  const { openNew, openCurrent } = useTabs.getState();
  const { status } = listenerStore.getState().live;
  const open = behavior === "new" ? openNew : openCurrent;

  if (status === "active") {
    open({ type: "sessions", id: sessionId });
    return;
  }

  open({
    type: "sessions",
    id: sessionId,
    state: { view: null, autoStart: true },
  });
}

const AUDIO_FILTERS = [
  { name: "Audio", extensions: ["wav", "mp3", "ogg", "mp4", "m4a", "flac"] },
];
const TRANSCRIPT_FILTERS = [{ name: "Transcript", extensions: ["vtt", "srt"] }];

export function useNewNoteAndUpload() {
  const openNew = useTabs((state) => state.openNew);

  const handler = useCallback(
    async (kind: "audio" | "transcript") => {
      const defaultPath = await downloadDir();
      const selection = await selectFile({
        title: kind === "audio" ? t`Upload Audio` : t`Upload Transcript`,
        multiple: false,
        directory: false,
        defaultPath,
        filters: kind === "audio" ? AUDIO_FILTERS : TRANSCRIPT_FILTERS,
      });

      const filePath = Array.isArray(selection) ? selection[0] : selection;
      if (!filePath) {
        return;
      }

      const reservation = reservePendingUpload({ kind, filePath });
      if (!reservation) {
        sonnerToast.error(
          t`Too many uploads are waiting. Open an existing upload and try again.`,
        );
        return;
      }

      try {
        const sessionId = await createNoteSession();
        if (!reservation.commit(sessionId)) {
          sonnerToast.error(
            t`Could not prepare this upload. Please try again.`,
          );
          return;
        }
        openNew({
          type: "sessions",
          id: sessionId,
          state: { view: null, autoStart: null },
        });
      } catch (error) {
        reservation.cancel();
        throw error;
      }
    },
    [openNew],
  );

  return handler;
}
