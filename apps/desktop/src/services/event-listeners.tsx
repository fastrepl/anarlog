import { type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { events as notificationEvents } from "@hypr/plugin-notification";
import {
  commands as updaterCommands,
  events as updaterEvents,
} from "@hypr/plugin-updater2";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import {
  useCreateSession,
  useGetOrCreateSessionForEvent,
} from "~/session/hooks/storage";
import * as settings from "~/store/tinybase/store/settings";
import { useTabs } from "~/store/zustand/tabs";

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
  const createSession = useCreateSession();
  const getOrCreateSessionForEvent = useGetOrCreateSessionForEvent();
  const settingsStore = settings.UI.useStore(settings.STORE_ID);
  const openNew = useTabs((state) => state.openNew);
  const pendingAutoStart = useRef<{ eventId: string | null } | null>(null);
  const createSessionRef = useRef(createSession);
  const getOrCreateSessionForEventRef = useRef(getOrCreateSessionForEvent);
  const settingsStoreRef = useRef(settingsStore);
  const openNewRef = useRef(openNew);

  createSessionRef.current = createSession;
  getOrCreateSessionForEventRef.current = getOrCreateSessionForEvent;
  settingsStoreRef.current = settingsStore;
  openNewRef.current = openNew;

  useEffect(() => {
    if (pendingAutoStart.current) {
      const { eventId } = pendingAutoStart.current;
      pendingAutoStart.current = null;
      const sessionId = eventId
        ? (getOrCreateSessionForEventRef.current(eventId) ?? null)
        : createSessionRef.current();
      if (!sessionId) {
        return;
      }
      openNew({
        type: "sessions",
        id: sessionId,
        state: { view: null, autoStart: true },
      });
    }
  }, [createSession, getOrCreateSessionForEvent, openNew]);

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
          const sessionId = eventId
            ? getOrCreateSessionForEventRef.current(eventId)
            : createSessionRef.current();
          if (!sessionId) {
            pendingAutoStart.current = { eventId };
            return;
          }
          openNewRef.current({
            type: "sessions",
            id: sessionId,
            state: { view: null, autoStart: true },
          });
        } else if (payload.type === "notification_option_selected") {
          const selectedIndex = payload.selected_index;
          const eventIds =
            payload.source?.type === "mic_detected"
              ? (payload.source.event_ids ?? [])
              : [];
          const eventId =
            selectedIndex < eventIds.length ? eventIds[selectedIndex] : null;

          const sessionId =
            eventId !== null
              ? getOrCreateSessionForEventRef.current(eventId)
              : createSessionRef.current();
          if (!sessionId) {
            pendingAutoStart.current = { eventId };
            return;
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
