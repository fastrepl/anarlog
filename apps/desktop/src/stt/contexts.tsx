import React, { createContext, useContext, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import { events as detectEvents } from "@hypr/plugin-detect";
import { commands as notificationCommands } from "@hypr/plugin-notification";

import * as main from "~/store/tinybase/store/main";
import {
  createListenerStore,
  type ListenerStore,
} from "~/store/zustand/listener";

const ListenerContext = createContext<ListenerStore | null>(null);

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

  const tinybaseStoreRef = useRef(tinybaseStore);
  useEffect(() => {
    tinybaseStoreRef.current = tinybaseStore;
  }, [tinybaseStore]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

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

          const options =
            nearbyEvents.length > 0 ? nearbyEvents.map((e) => e.title) : null;

          void notificationCommands.showNotification({
            key: payload.key,
            title: "Meeting in progress?",
            message:
              "Noticed microphone usage for certain period of time. Start listening?",
            timeout: { secs: 15, nanos: 0 },
            source: {
              type: "mic_detected",
              app_names: payload.apps.map((a) => a.name),
              event_ids: nearbyEvents.map((e) => e.id),
            },
            start_time: null,
            participants: null,
            event_details: null,
            action_label: null,
            options,
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
  }, [stop, setMuted]);
};
