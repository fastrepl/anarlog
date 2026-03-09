import { useRouteContext } from "@tanstack/react-router";
import { useCallback } from "react";
import { useShallow } from "zustand/shallow";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { id } from "~/shared/utils";
import { useTabs } from "~/store/zustand/tabs";

export function useNewNote({
  behavior = "new",
}: {
  behavior?: "new" | "current";
}) {
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

export function useNewNoteAndListen() {
  const { persistedStore, internalStore } = useRouteContext({
    from: "__root__",
  });
  const openNew = useTabs((state) => state.openNew);

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

    openNew({
      type: "sessions",
      id: sessionId,
      state: { view: null, autoStart: true },
    });
  }, [persistedStore, internalStore, openNew]);

  return handler;
}
