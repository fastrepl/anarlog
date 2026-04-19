import { useEffect } from "react";

import { reconcileCalendarSessions } from "./calendar/reconcile";

import * as main from "~/store/tinybase/store/main";

const RECONCILE_DEBOUNCE_MS = 50;

/**
 * Keeps `sessions.event_json` / `events.participants_json` in sync with
 * Rust-owned calendar data. Reconcile after TinyBase actually ingests updated
 * `events` / `calendars` rows so we don't race the file-backed persisters.
 *
 * The file-backed tables can land in separate callbacks, so debounce to collapse
 * near-simultaneous `events.json` / `calendars.json` reloads into a single pass.
 */
export function CalendarSyncReconciler() {
  const store = main.UI.useStore(main.STORE_ID);

  useEffect(() => {
    if (!store) {
      return;
    }

    let timeout: ReturnType<typeof setTimeout> | null = null;
    const scheduleReconcile = () => {
      if (timeout) {
        clearTimeout(timeout);
      }
      timeout = setTimeout(() => {
        timeout = null;
        reconcileCalendarSessions(store as main.Store);
      }, RECONCILE_DEBOUNCE_MS);
    };

    const listenerIds = [
      store.addTableListener("events", scheduleReconcile),
      store.addTableListener("calendars", scheduleReconcile),
    ];

    scheduleReconcile();

    return () => {
      if (timeout) {
        clearTimeout(timeout);
      }
      for (const listenerId of listenerIds) {
        store.delListener(listenerId);
      }
    };
  }, [store]);

  return null;
}
