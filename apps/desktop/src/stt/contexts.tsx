import React, { createContext, useContext, useRef } from "react";
import { useStore } from "zustand";
import { useShallow } from "zustand/shallow";

import { useHandleDetectEvents } from "./detect-events";

import {
  createListenerStore,
  type ListenerStore,
} from "~/store/zustand/listener";

export {
  AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS,
  AUTO_STOP_CONFIRM_DELAY_MS,
  AUTO_STOP_EVENT_END_GRACE_MS,
  AUTO_STOP_NETWORK_HOLD_MS,
  AUTO_STOP_RECENT_OFFLINE_MS,
} from "./auto-stop";

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
