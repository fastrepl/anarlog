import { useRouteContext } from "@tanstack/react-router";
import { downloadDir } from "@tauri-apps/api/path";
import { open as selectFile } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import { useShallow } from "zustand/shallow";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { id } from "~/shared/utils";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { setPendingUpload } from "~/stt/pending-upload";

export function useNewNote({
  behavior = "new",
}: {
  behavior?: "new" | "current";
} = {}) {
  const { persistedStore, internalStore } = useRouteContext({
    from: "__root__",
  });
  const { openNew, openCurrent } = useTabs(
    useShallow((state) => ({
      openNew: state.openNew,
      openCurrent: state.openCurrent,
    })),
  );

  const handler = useCallback(() => {
    const user_id = internalStore?.getValue("user_id");
    const sessionId = id();

    persistedStore?.setRow("sessions", sessionId, {
      user_id,
      created_at: new Date().toISOString(),
      title: "",
    });

    void analyticsCommands.event({
      event: "note_created",
      has_event_id: false,
    });

    const ff = behavior === "new" ? openNew : openCurrent;
    ff({ type: "sessions", id: sessionId });
  }, [persistedStore, internalStore, openNew, openCurrent, behavior]);

  return handler;
}

export function useNewNoteAndListen({
  behavior = "new",
}: {
  behavior?: "new" | "current";
} = {}) {
  const { persistedStore, internalStore } = useRouteContext({
    from: "__root__",
  });
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
    if ((status === "active" || status === "finalizing") && liveSessionId) {
      const ff = behavior === "new" ? openNew : openCurrent;
      ff({ type: "sessions", id: liveSessionId });
      return;
    }

    const user_id = internalStore?.getValue("user_id");
    const sessionId = id();

    persistedStore?.setRow("sessions", sessionId, {
      user_id,
      created_at: new Date().toISOString(),
      title: "",
    });

    void analyticsCommands.event({
      event: "note_created",
      has_event_id: false,
    });

    const ff = behavior === "new" ? openNew : openCurrent;
    ff({
      type: "sessions",
      id: sessionId,
      state: { view: null, autoStart: true },
    });
  }, [
    status,
    liveSessionId,
    persistedStore,
    internalStore,
    openNew,
    openCurrent,
    behavior,
  ]);

  return handler;
}

const AUDIO_FILTERS = [
  { name: "Audio", extensions: ["wav", "mp3", "ogg", "mp4", "m4a", "flac"] },
];
const TRANSCRIPT_FILTERS = [{ name: "Transcript", extensions: ["vtt", "srt"] }];

export function useNewNoteAndUpload() {
  const { persistedStore, internalStore } = useRouteContext({
    from: "__root__",
  });
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

      const user_id = internalStore?.getValue("user_id");
      const sessionId = id();

      persistedStore?.setRow("sessions", sessionId, {
        user_id,
        created_at: new Date().toISOString(),
        title: "",
      });

      void analyticsCommands.event({
        event: "note_created",
        has_event_id: false,
      });

      setPendingUpload(sessionId, { kind, filePath });
      openNew({
        type: "sessions",
        id: sessionId,
        state: { view: null, autoStart: null },
      });
    },
    [persistedStore, internalStore, openNew],
  );

  return handler;
}
