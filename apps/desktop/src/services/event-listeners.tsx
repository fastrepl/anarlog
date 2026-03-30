import { type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import {
  events as notificationEvents,
  type NotificationSource,
} from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { parseSummaryReadyNotificationKey } from "./summary-ready-notification";

import * as main from "~/store/tinybase/store/main";
import {
  createSession,
  getOrCreateSessionForEventId,
} from "~/store/tinybase/store/sessions";
import { useTabs } from "~/store/zustand/tabs";

type NotificationTarget = {
  key: string;
  source: NotificationSource | null;
};

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

export function getNotificationOpenConfig(
  notification: NotificationTarget,
  store: MainStore,
) {
  const summaryTarget = parseSummaryReadyNotificationKey(notification.key);
  if (summaryTarget) {
    return {
      id: summaryTarget.sessionId,
      state: {
        view: { type: "enhanced" as const, id: summaryTarget.enhancedNoteId },
        autoStart: null,
      },
    };
  }

  const eventId =
    notification.source?.type === "calendar_event"
      ? notification.source.event_id
      : null;
  const sessionId = eventId
    ? getOrCreateSessionForEventId(store, eventId)
    : createSession(store);

  return {
    id: sessionId,
    state: { view: null, autoStart: true },
  };
}

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
  const pendingNotification = useRef<NotificationTarget | null>(null);
  const storeRef = useRef(store);
  const openNewRef = useRef(openNew);

  useEffect(() => {
    storeRef.current = store;
    openNewRef.current = openNew;
  }, [store, openNew]);

  useEffect(() => {
    if (pendingNotification.current && store) {
      const notification = pendingNotification.current;
      pendingNotification.current = null;
      const { id, state } = getNotificationOpenConfig(notification, store);
      openNew({
        type: "sessions",
        id,
        state,
      });
    }
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
          const currentStore = storeRef.current;
          if (!currentStore) {
            pendingNotification.current = {
              key: payload.key,
              source: payload.source,
            };
            return;
          }
          const { id, state } = getNotificationOpenConfig(
            { key: payload.key, source: payload.source },
            currentStore,
          );
          openNewRef.current({
            type: "sessions",
            id,
            state,
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

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: true },
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

export function EventListeners() {
  useUpdaterEvents();
  useNotificationEvents();

  return null;
}
