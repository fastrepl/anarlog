import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import { useShallow } from "zustand/shallow";

import { createSession } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
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
  const { openNew, openCurrent } = useTabs(
    useShallow((state) => ({
      openNew: state.openNew,
      openCurrent: state.openCurrent,
    })),
  );
  const { status, sessionId: liveSessionId } = useListener((state) => ({
    status: state.live.status,
    sessionId: state.live.sessionId,
  }));

  const handler = useCallback(() => {
    if (status === "active" && liveSessionId) {
      const ff = behavior === "new" ? openNew : openCurrent;
      ff({ type: "sessions", id: liveSessionId });
      return;
    }

    const ff = behavior === "new" ? openNew : openCurrent;
    void createSession()
      .then((sessionId) => {
        ff({
          type: "sessions",
          id: sessionId,
          state: { view: null, autoStart: true },
        });
      })
      .catch((error) => {
        console.error("[session] failed to create listening note", error);
      });
  }, [status, liveSessionId, openNew, openCurrent, behavior]);

  return handler;
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
