import { useEffect, useRef } from "react";
import type { Queries } from "tinybase/with-schemas";
import {
  useScheduleTaskRun,
  useScheduleTaskRunCallback,
  useSetTask,
} from "tinytick/ui-react";

import { events as appleCalendarEvents } from "@hypr/plugin-calendar";

import { CALENDAR_SYNC_TASK_ID, syncCalendarEvents } from "./calendar";
import {
  checkEventNotifications,
  EVENT_NOTIFICATION_INTERVAL,
  EVENT_NOTIFICATION_TASK_ID,
  type NotifiedEventsMap,
} from "./event-notification";

import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";

const CALENDAR_SYNC_INTERVAL = 60 * 1000; // 60 sec

export function TaskManager() {
  const store = main.UI.useStore(main.STORE_ID);
  const queries = main.UI.useQueries(main.STORE_ID);

  const settingsStore = settings.UI.useStore(settings.STORE_ID);
  const notifiedEventsRef = useRef<NotifiedEventsMap>(new Map());

  useSetTask(CALENDAR_SYNC_TASK_ID, async () => {
    await syncCalendarEvents(
      store as main.Store,
      queries as Queries<main.Schemas>,
    );
  }, [store, queries, settingsStore]);

  useScheduleTaskRun(CALENDAR_SYNC_TASK_ID, undefined, 0, {
    repeatDelay: CALENDAR_SYNC_INTERVAL,
  });

  const scheduleCalendarSync = useScheduleTaskRunCallback(
    CALENDAR_SYNC_TASK_ID,
    undefined,
    0,
  );

  useEffect(() => {
    const unlisten = appleCalendarEvents.calendarChangedEvent.listen(() => {
      scheduleCalendarSync();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [scheduleCalendarSync]);

  useSetTask(EVENT_NOTIFICATION_TASK_ID, async () => {
    if (!store || !settingsStore) return;
    checkEventNotifications(
      store as main.Store,
      settingsStore as settings.Store,
      notifiedEventsRef.current,
    );
  }, [store, settingsStore]);

  useScheduleTaskRun(EVENT_NOTIFICATION_TASK_ID, undefined, 0, {
    repeatDelay: EVENT_NOTIFICATION_INTERVAL,
  });

  return null;
}
