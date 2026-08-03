import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import { useShallow } from "zustand/shallow";

import { createSession } from "~/session/queries";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { setPendingUpload } from "~/stt/pending-upload";

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
    void createSession()
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

  void createSession()
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
        title: kind === "audio" ? "Upload Audio" : "Upload Transcript",
        multiple: false,
        directory: false,
        defaultPath,
        filters: kind === "audio" ? AUDIO_FILTERS : TRANSCRIPT_FILTERS,
      });

      const filePath = Array.isArray(selection) ? selection[0] : selection;
      if (!filePath) {
        return;
      }

      const sessionId = await createSession();
      setPendingUpload(sessionId, { kind, filePath });
      openNew({
        type: "sessions",
        id: sessionId,
        state: { view: null, autoStart: null },
      });
    },
    [openNew],
  );

  return handler;
}
