import React, { createContext, useContext, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import {
  commands as detectCommands,
  events as detectEvents,
} from "@hypr/plugin-detect";
import { commands as notificationCommands } from "@hypr/plugin-notification";

import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";
import {
  createListenerStore,
  type ListenerStore,
} from "~/store/zustand/listener";

const ListenerContext = createContext<ListenerStore | null>(null);
export const AUTO_STOP_CONFIRM_DELAY_MS = 5_000;

function getIgnorableAppIds(apps: { id: string }[]) {
  return [
    ...new Set(
      apps.map((app) => app.id).filter((id) => id && !id.startsWith("pid:")),
    ),
  ];
}

export const ListenerProvider = ({
  children,
  store,
}: {
  children: React.ReactNode;
  store: ListenerStore;
}) => {
  useHandleDetectEvents(store);

  const storeRef = useRef<ListenerStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = store;
  }

  return (
    <ListenerContext.Provider value={storeRef.current}>
      {children}
    </ListenerContext.Provider>
  );
};

export const useListener = <T,>(
  selector: Parameters<
    typeof useStore<ReturnType<typeof createListenerStore>, T>
  >[1],
) => {
  const store = useContext(ListenerContext);

  if (!store) {
    throw new Error("'useListener' must be used within a 'ListenerProvider'");
  }

  return useStore(store, useShallow(selector));
};

function getNearbyEvents(
  tinybaseStore: NonNullable<ReturnType<typeof main.UI.useStore>>,
): { id: string; title: string }[] {
  const now = Date.now();
  const windowMs = 15 * 60 * 1000;
  const results: { id: string; title: string; startedAt: number }[] = [];

  tinybaseStore.forEachRow("events", (eventId, _forEachCell) => {
    const event = tinybaseStore.getRow("events", eventId);
    if (!event?.started_at) return;
    if (event.is_all_day) return;

    const startTime = new Date(String(event.started_at)).getTime();
    if (isNaN(startTime)) return;

    if (Math.abs(startTime - now) <= windowMs) {
      results.push({
        id: eventId,
        title: String(event.title || "Untitled Event"),
        startedAt: startTime,
      });
    }
  });

  results.sort((a, b) => a.startedAt - b.startedAt);
  return results.map(({ id, title }) => ({ id, title }));
}

const useHandleDetectEvents = (store: ListenerStore) => {
  const stop = useStore(store, (state) => state.stop);
  const setMuted = useStore(store, (state) => state.setMuted);
  const tinybaseStore = main.UI.useStore(main.STORE_ID);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);

  const tinybaseStoreRef = useRef(tinybaseStore);
  const settingsStoreRef = useRef(settingsStore);
  const pendingAutoStopRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    tinybaseStoreRef.current = tinybaseStore;
    settingsStoreRef.current = settingsStore;
  }, [tinybaseStore, settingsStore]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const clearPendingAutoStop = () => {
      if (pendingAutoStopRef.current) {
        clearTimeout(pendingAutoStopRef.current);
        pendingAutoStopRef.current = null;
      }
    };

    const confirmAutoStop = async (stoppedTriggerAppIds: string[]) => {
      if (store.getState().live.status !== "active") {
        return;
      }

      const currentTrigger = store.getState().live.triggerAppIds;
      if (
        !currentTrigger ||
        !stoppedTriggerAppIds.some((id) => currentTrigger.includes(id))
      ) {
        return;
      }

      const result = await detectCommands.listMicUsingApplications();
      if (result.status === "ok") {
        const activeAppIds = new Set(result.data.map((app) => app.id));
        if (stoppedTriggerAppIds.some((id) => activeAppIds.has(id))) {
          return;
        }
      }

      stop();
    };

    detectEvents.detectEvent
      .listen(({ payload }) => {
        if (payload.type === "micDetected") {
          if (store.getState().live.status === "active") {
            return;
          }

          const currentTinybaseStore = tinybaseStoreRef.current;
          const nearbyEvents = currentTinybaseStore
            ? getNearbyEvents(currentTinybaseStore)
            : [];
          const ignorableAppIds = getIgnorableAppIds(payload.apps);

          const options =
            nearbyEvents.length > 0 ? nearbyEvents.map((e) => e.title) : null;
          const footer =
            ignorableAppIds.length > 0
              ? {
                  text:
                    ignorableAppIds.length === 1
                      ? "Ignore this app?"
                      : "Ignore these apps?",
                  actionLabel: "Yes",
                }
              : null;

          void notificationCommands.showNotification({
            key: payload.key,
            title: "Are you in a meeting?",
            message: "",
            timeout: { secs: 15, nanos: 0 },
            source: {
              type: "mic_detected",
              app_names: payload.apps.map((a) => a.name),
              app_ids: ignorableAppIds,
              event_ids: nearbyEvents.map((e) => e.id),
            },
            start_time: null,
            participants: null,
            event_details: null,
            action_label: null,
            options,
            footer,
            icon: null,
          });
        } else if (payload.type === "micStopped") {
          const autoStopEnabled =
            settingsStoreRef.current?.getValue("auto_stop_meetings") !== false;
          if (!autoStopEnabled) {
            return;
          }

          const trigger = store.getState().live.triggerAppIds;
          const stoppedTriggerAppIds =
            trigger?.filter((id) =>
              payload.apps.some((app) => app.id === id),
            ) ?? [];
          if (stoppedTriggerAppIds.length > 0) {
            clearPendingAutoStop();
            pendingAutoStopRef.current = setTimeout(() => {
              pendingAutoStopRef.current = null;
              void confirmAutoStop(stoppedTriggerAppIds);
            }, AUTO_STOP_CONFIRM_DELAY_MS);
          }
        } else if (payload.type === "sleepStateChanged") {
          if (payload.value) {
            clearPendingAutoStop();
            stop();
          }
        } else if (payload.type === "micMuted") {
          setMuted(payload.value);
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        console.error("Failed to setup detect event listener:", err);
      });

    return () => {
      cancelled = true;
      clearPendingAutoStop();
      unlisten?.();
    };
  }, [stop, setMuted, store]);
};
