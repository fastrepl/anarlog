import { useEffect } from "react";

import { reconcileCalendarSessions } from "./calendar/reconcile";

import * as main from "~/store/tinybase/store/main";

export function CalendarSyncReconciler() {
  const store = main.UI.useStore(main.STORE_ID);
  const calendars = main.UI.useTable("calendars", main.STORE_ID);
  const events = main.UI.useTable("events", main.STORE_ID);

  useEffect(() => {
    if (!store) {
      return;
    }

    reconcileCalendarSessions(store as main.Store);
  }, [store, calendars, events]);

  return null;
}
