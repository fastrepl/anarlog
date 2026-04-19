import { useEffect } from "react";

import { events as calendarEvents } from "@hypr/plugin-calendar";

import { reconcileCalendarSessions } from "./calendar/reconcile";

import * as main from "~/store/tinybase/store/main";

/**
 * Keeps `sessions.event_json` / `events.participants_json` in sync with
 * Rust-owned calendar data. Fires once on mount (for initial persister loads)
 * and on every completed Rust sync pass that actually changed something.
 *
 * This deliberately does NOT subscribe to `useTable("calendars" | "events")`
 * — those fire on every cell mutation and would thrash the reconciler
 * hundreds of times during a single sync pass. One reconcile per completed
 * sync is the correct granularity.
 */
export function CalendarSyncReconciler() {
  const store = main.UI.useStore(main.STORE_ID);

  useEffect(() => {
    if (!store) {
      return;
    }

    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void (async () => {
      try {
        const fn = await calendarEvents.calendarSyncEvent.listen(
          ({ payload }) => {
            if (payload.type === "syncFinished" && payload.data_changed) {
              reconcileCalendarSessions(store as main.Store);
            }
          },
        );
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      } catch (error) {
        console.error("[CalendarSyncReconciler] failed to listen:", error);
      }
    })();

    // Harmless one-shot: covers the case where the TinyBase persister has
    // already loaded `events.json` before the worker's next `syncFinished`
    // fires. If the table is still empty (persister hasn't resolved yet)
    // this is a no-op; the next `syncFinished` will do the work.
    reconcileCalendarSessions(store as main.Store);

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [store]);

  return null;
}
