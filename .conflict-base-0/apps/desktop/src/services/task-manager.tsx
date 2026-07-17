import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import {
  useScheduleTaskRun,
  useScheduleTaskRunCallback,
  useSetTask,
} from "tinytick/ui-react";

import { events as appleCalendarEvents } from "@hypr/plugin-calendar";

import {
  AUDIO_RETENTION_INTERVAL,
  AUDIO_RETENTION_TASK_ID,
  cleanupExpiredAudio,
  normalizeAudioRetention,
} from "./audio-retention";
import { CALENDAR_SYNC_TASK_ID, syncCalendarEvents } from "./calendar";
import {
  checkEventNotifications,
  EVENT_NOTIFICATION_INTERVAL,
  EVENT_NOTIFICATION_TASK_ID,
  type NotifiedEventsMap,
} from "./event-notification";

import { useConfigValue } from "~/shared/config";

const CALENDAR_SYNC_INTERVAL = 60 * 1000; // 60 sec

export function TaskManager() {
  const queryClient = useQueryClient();

  const notificationEvent = useConfigValue("notification_event");
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const notifiedEventsRef = useRef<NotifiedEventsMap>(new Map());

  useSetTask(CALENDAR_SYNC_TASK_ID, async () => {
    await syncCalendarEvents();
  }, []);

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
    await checkEventNotifications(notificationEvent, notifiedEventsRef.current);
  }, [notificationEvent]);

  useScheduleTaskRun(EVENT_NOTIFICATION_TASK_ID, undefined, 0, {
    repeatDelay: EVENT_NOTIFICATION_INTERVAL,
  });

  useSetTask(AUDIO_RETENTION_TASK_ID, async () => {
    const deletedSessionIds = await cleanupExpiredAudio(audioRetention);
    for (const sessionId of deletedSessionIds) {
      void queryClient.invalidateQueries({
        queryKey: ["audio", sessionId, "exist"],
      });
      void queryClient.invalidateQueries({
        queryKey: ["audio", sessionId, "url"],
      });
    }
  }, [audioRetention, queryClient]);

  useScheduleTaskRun(AUDIO_RETENTION_TASK_ID, undefined, 0, {
    repeatDelay: AUDIO_RETENTION_INTERVAL,
  });

  return null;
}
