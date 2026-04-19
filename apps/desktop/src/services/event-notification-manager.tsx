import { useEffect, useRef } from "react";

import {
  checkEventNotifications,
  type NotifiedEventsMap,
} from "./event-notification";

import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";

export function EventNotificationManager() {
  const store = main.UI.useStore(main.STORE_ID);
  const settingsStore = settings.UI.useStore(settings.STORE_ID);
  const notifiedEventsRef = useRef<NotifiedEventsMap>(new Map());

  useEffect(() => {
    if (!store || !settingsStore) {
      return;
    }

    const run = () => {
      checkEventNotifications(
        store as main.Store,
        settingsStore as settings.Store,
        notifiedEventsRef.current,
      );
    };

    run();
    const interval = window.setInterval(run, 30 * 1000);
    return () => {
      window.clearInterval(interval);
    };
  }, [store, settingsStore]);

  return null;
}
