import React, { createContext, useContext, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import { events as detectEvents } from "@hypr/plugin-detect";
import { commands as notificationCommands } from "@hypr/plugin-notification";

import { useNearbyEventsForMicPrompt } from "~/session/hooks/runtime";
import {
  createListenerStore,
  type ListenerStore,
} from "~/store/zustand/listener";

const ListenerContext = createContext<ListenerStore | null>(null);

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

const useHandleDetectEvents = (store: ListenerStore) => {
  const stop = useStore(store, (state) => state.stop);
  const setMuted = useStore(store, (state) => state.setMuted);
  const getNearbyEvents = useNearbyEventsForMicPrompt();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    detectEvents.detectEvent
      .listen(({ payload }) => {
        if (payload.type === "micDetected") {
          if (store.getState().live.status === "active") {
            return;
          }

          const nearbyEvents = getNearbyEvents();
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
          stop();
        } else if (payload.type === "sleepStateChanged") {
          if (payload.value) {
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
      unlisten?.();
    };
  }, [getNearbyEvents, stop, setMuted]);
};
