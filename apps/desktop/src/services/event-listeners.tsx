import { type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { events as notificationEvents } from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import * as main from "~/store/tinybase/store/main";
import {
  createSession,
  getOrCreateSessionForEventId,
} from "~/store/tinybase/store/sessions";
import * as settings from "~/store/tinybase/store/settings";
import { listenerStore } from "~/store/zustand/listener/instance";
import { useTabs } from "~/store/zustand/tabs";
import { parseAutoStopEndedNotificationKey } from "~/stt/auto-stop-notification";
import { parseBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

function parseIgnoredPlatforms(value: unknown) {
  if (typeof value !== "string") {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter(
          (bundleId): bundleId is string => typeof bundleId === "string",
        )
      : [];
  } catch {
    return [];
  }
}

function shouldAutoStartNotificationSession(
  store: MainStore,
  eventId: string | null,
  triggerAppIds: string[] | null,
): boolean {
  if (triggerAppIds && triggerAppIds.length > 0) {
    return true;
  }

  if (!eventId) {
    return true;
  }

  const startedAt = store.getRow("events", eventId)?.started_at;
  if (!startedAt) {
    return false;
  }

  const startTime = new Date(String(startedAt)).getTime();
  return !Number.isNaN(startTime) && startTime <= Date.now();
}

function handleAutoStopEndedNotification(
  type: "notification_confirm" | "notification_accept",
  key: string,
): boolean {
  const sessionId = parseAutoStopEndedNotificationKey(key);
  if (!sessionId) {
    return false;
  }

  if (type !== "notification_accept") {
    return true;
  }

  const listenerState = listenerStore.getState();
  if (
    listenerState.live.status === "active" &&
    listenerState.live.sessionId === sessionId
  ) {
    listenerState.stop();
  }

  return true;
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
  const settingsStore = settings.UI.useStore(settings.STORE_ID);
  const openNew = useTabs((state) => state.openNew);
  const pendingAutoStart = useRef<{
    eventId: string | null;
    triggerAppIds: string[] | null;
  } | null>(null);
  const storeRef = useRef(store);
  const settingsStoreRef = useRef(settingsStore);
  const openNewRef = useRef(openNew);

  useEffect(() => {
    storeRef.current = store;
    settingsStoreRef.current = settingsStore;
    openNewRef.current = openNew;
  }, [store, settingsStore, openNew]);

  useEffect(() => {
    if (pendingAutoStart.current && store) {
      const { eventId, triggerAppIds } = pendingAutoStart.current;
      pendingAutoStart.current = null;
      const sessionId = eventId
        ? getOrCreateSessionForEventId(store, eventId)
        : createSession(store);

      if (triggerAppIds && triggerAppIds.length > 0) {
        listenerStore.getState().setTriggerAppIds(triggerAppIds);
      }
      const autoStart = shouldAutoStartNotificationSession(
        store,
        eventId,
        triggerAppIds,
      );

      openNew({
        type: "sessions",
        id: sessionId,
        state: { view: null, autoStart: autoStart ? true : null },
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
          if (handleAutoStopEndedNotification(payload.type, payload.key)) {
            return;
          }

          const eventId =
            payload.source?.type === "calendar_event"
              ? payload.source.event_id
              : null;
          const sourceSessionId =
            payload.source?.type === "session"
              ? payload.source.session_id
              : parseBatchCompletedNotificationKey(payload.key);
          const triggerAppIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.app_ids ?? null)
              : null;
          const currentStore = storeRef.current;
          if (sourceSessionId) {
            openNewRef.current({
              type: "sessions",
              id: sourceSessionId,
              state: { view: null, autoStart: null },
            });
            return;
          }

          if (!currentStore) {
            pendingAutoStart.current = { eventId, triggerAppIds };
            return;
          }
          const sessionId = eventId
            ? getOrCreateSessionForEventId(currentStore, eventId)
            : createSession(currentStore);

          if (triggerAppIds && triggerAppIds.length > 0) {
            listenerStore.getState().setTriggerAppIds(triggerAppIds);
          }
          const autoStart = shouldAutoStartNotificationSession(
            currentStore,
            eventId,
            triggerAppIds,
          );

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: autoStart ? true : null },
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

          if (payload.source?.type === "mic_detected") {
            const triggerAppIds = payload.source.app_ids ?? [];
            listenerStore
              .getState()
              .setTriggerAppIds(
                triggerAppIds.length > 0 ? triggerAppIds : null,
              );
          }

          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: true },
          });
        } else if (payload.type === "notification_footer_action") {
          if (payload.source?.type !== "mic_detected") {
            return;
          }

          const currentSettingsStore = settingsStoreRef.current;
          if (!currentSettingsStore) {
            return;
          }

          const appIds = payload.source.app_ids ?? [];
          if (appIds.length === 0) {
            return;
          }

          const ignoredPlatforms = parseIgnoredPlatforms(
            currentSettingsStore.getValue("ignored_platforms"),
          );
          const nextIgnoredPlatforms = [
            ...new Set([...ignoredPlatforms, ...appIds]),
          ];

          if (nextIgnoredPlatforms.length === ignoredPlatforms.length) {
            return;
          }

          currentSettingsStore.setValue(
            "ignored_platforms",
            JSON.stringify(nextIgnoredPlatforms),
          );
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
