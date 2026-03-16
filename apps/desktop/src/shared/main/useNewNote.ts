import { useRouteContext } from "@tanstack/react-router";
import { useCallback } from "react";
import { useShallow } from "zustand/shallow";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { id } from "~/shared/utils";
import { useTabs } from "~/store/zustand/tabs";
import type { SessionsState } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export function useCreateSession({
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

  return useCallback(
    (state?: SessionsState | null) => {
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
        state: state ?? undefined,
      });

      return sessionId;
    },
    [persistedStore, internalStore, openNew, openCurrent, behavior],
  );
}

export function useNewNote({
  behavior = "new",
}: {
  behavior?: "new" | "current";
}) {
  const createSession = useCreateSession({ behavior });

  return useCallback(() => {
    createSession();
  }, [createSession]);
}

export function useNewNoteAndListen({
  behavior = "new",
}: {
  behavior?: "new" | "current";
} = {}) {
  const createSession = useCreateSession({ behavior });
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

  return useCallback(() => {
    if ((status === "active" || status === "finalizing") && liveSessionId) {
      const ff = behavior === "new" ? openNew : openCurrent;
      ff({ type: "sessions", id: liveSessionId });
      return;
    }

    createSession({ view: null, autoStart: true });
  }, [behavior, createSession, liveSessionId, openCurrent, openNew, status]);
}
