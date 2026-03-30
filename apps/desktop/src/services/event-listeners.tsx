import { type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { events as calendarEvents } from "@hypr/plugin-calendar";
import { events as notificationEvents } from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import {
  consumePendingCalendarAutoStarts,
  resolveCalendarAutoStartEvent,
} from "./calendar-auto-start";

import * as main from "~/store/tinybase/store/main";
import {
  createSession,
  getOrCreateSessionForEventId,
} from "~/store/tinybase/store/sessions";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";

function useUpdaterEvents() {
  const openNew = useTabs((state) => state.openNew);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;

    void updaterEvents.updatedEvent
      .listen(({ payload: { previous, current } }) => {
        openNew({
          type: "changelog",
          state: { previous, current },
        });
      })
      .then((f) => {
        unlisten = f;
        updaterCommands.maybeEmitUpdated();
      });

    return () => {
      unlisten?.();
    };
  }, [openNew]);
}

function useNotificationEvents() {
  const store = main.UI.useStore(main.STORE_ID);
  const openNew = useTabs((state) => state.openNew);
  const storeRef = useRef(store);
  const openNewRef = useRef(openNew);

  useEffect(() => {
    storeRef.current = store;
    openNewRef.current = openNew;
  }, [store, openNew]);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    void notificationEvents.notificationEvent
      .listen(({ payload }) => {
        if (
          payload.type === "notification_confirm" ||
          payload.type === "notification_accept"
        ) {
          const eventId =
            payload.source?.type === "calendar_event"
              ? payload.source.event_id
              : null;
          const currentStore = storeRef.current;
          if (!currentStore) return;
          const sessionId = eventId
            ? getOrCreateSessionForEventId(currentStore, eventId)
            : createSession(currentStore);
          listenerStore.getState().requestAutoStart(sessionId);
          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null },
          });
        } else if (payload.type === "notification_option_selected") {
          const currentStore = storeRef.current;
          if (!currentStore) return;

          const selectedIndex = payload.selected_index;
          const eventIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.event_ids ?? [])
              : [];

          const sessionId =
            selectedIndex < eventIds.length
              ? getOrCreateSessionForEventId(
                  currentStore,
                  eventIds[selectedIndex],
                )
              : createSession(currentStore);

          listenerStore.getState().requestAutoStart(sessionId);
          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null },
          });
        }
      })
      .then((f) => {
        if (cancelled) {
          f();
        } else {
          unlisten = f;
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

function useCalendarStartEvents() {
  const store = main.UI.useStore(main.STORE_ID);
  const openNew = useTabs((state) => state.openNew);
  const pendingAutoStartTrackingIds = useRef(new Set<string>());
  const storeRef = useRef(store);
  const openNewRef = useRef(openNew);

  useEffect(() => {
    storeRef.current = store;
  }, [store]);
  useEffect(() => {
    openNewRef.current = openNew;
  }, [openNew]);

  const openCalendarAutoStart = (
    currentStore: NonNullable<typeof store>,
    eventRowId: string,
  ) => {
    const sessionId = getOrCreateSessionForEventId(currentStore, eventRowId);
    listenerStore.getState().requestAutoStart(sessionId);
    openNewRef.current({
      type: "sessions",
      id: sessionId,
      state: { view: null },
    });
  };

  const consumePendingAutoStarts = (
    currentStore: NonNullable<typeof store>,
  ) => {
    consumePendingCalendarAutoStarts(
      currentStore,
      pendingAutoStartTrackingIds.current,
      (eventRowId) => {
        openCalendarAutoStart(currentStore, eventRowId);
      },
    );
  };

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main" || !store) return;

    consumePendingAutoStarts(store);

    const listenerId = store.addRowListener("events", null, () => {
      consumePendingAutoStarts(store);
    });

    return () => {
      store.delListener(listenerId);
    };
  }, [store]);

  useEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") return;

    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    void calendarEvents.notificationWorkerEvent
      .listen(({ payload }) => {
        if (payload.type !== "eventStarted") return;

        const currentStore = storeRef.current;
        if (!currentStore) {
          pendingAutoStartTrackingIds.current.add(payload.event_id);
          return;
        }

        const resolution = resolveCalendarAutoStartEvent(
          currentStore,
          payload.event_id,
        );

        if (resolution.status === "pending") {
          pendingAutoStartTrackingIds.current.add(payload.event_id);
          return;
        }

        pendingAutoStartTrackingIds.current.delete(payload.event_id);

        if (resolution.status === "ready") {
          openCalendarAutoStart(currentStore, resolution.eventRowId);
        }
      })
      .then((f) => {
        if (cancelled) {
          f();
        } else {
          unlisten = f;
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

export function EventListeners() {
  useUpdaterEvents();
  useNotificationEvents();
  useCalendarStartEvents();

  return null;
}
