import { createListenerStore } from "./index";

type ListenerStoreSingleton = ReturnType<typeof createListenerStore>;

const LISTENER_STORE_KEY = "__hypr_listener_store__" as const;

const getListenerStore = (): ListenerStoreSingleton => {
  if (!import.meta.hot) {
    return createListenerStore();
  }

  const hotData = (import.meta.hot.data ?? {}) as {
    [LISTENER_STORE_KEY]?: ListenerStoreSingleton;
  };

  if (!hotData[LISTENER_STORE_KEY]) {
    hotData[LISTENER_STORE_KEY] = createListenerStore();
  }

  return hotData[LISTENER_STORE_KEY];
};

export const listenerStore = getListenerStore();
