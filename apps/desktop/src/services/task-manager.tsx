import { useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";

import { events as appleCalendarEvents } from "@anlg/plugin-calendar";

import {
  AUDIO_RETENTION_INTERVAL,
  AUDIO_RETENTION_TASK_ID,
  cleanupExpiredAudio,
  normalizeAudioRetention,
} from "./audio-retention";
import {
  CALENDAR_SYNC_TASK_ID,
  scheduleCalendarSync,
  syncCalendarEvents,
} from "./calendar";
import {
  checkEventNotifications,
  EVENT_NOTIFICATION_INTERVAL,
  EVENT_NOTIFICATION_TASK_ID,
  type NotifiedEventsMap,
} from "./event-notification";
import { cleanupExpiredVoiceprintCandidates } from "./voiceprint";

import {
  useRegisterTask,
  useScheduleTaskRun,
  useTaskScheduler,
} from "~/services/task-scheduler";
import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

const CALENDAR_SYNC_INTERVAL = 60 * 1000; // 60 sec
const CALENDAR_SYNC_MAX_DURATION = 120 * 1000; // 2 min

// Long-running tasks need explicit deadlines so a hung provider cannot block
// later deadline-driven work indefinitely.
const AUDIO_RETENTION_MAX_DURATION = 10 * 60 * 1000; // 10 min
const EVENT_NOTIFICATION_MAX_DURATION = 60 * 1000; // 60 sec
const REPEATING_TASK_MAX_RETRIES = 3;

export function TaskManager() {
  const queryClient = useQueryClient();
  const manager = useTaskScheduler();

  const notificationEvent = useConfigValue("notification_event");
  const notificationsDisabled = useConfigValue("notification_disabled");
  const audioRetention = normalizeAudioRetention(
    useConfigValue("audio_retention"),
  );
  const notifiedEventsRef = useRef<NotifiedEventsMap>(new Map());

  useRegisterTask(
    CALENDAR_SYNC_TASK_ID,
    async (_arg, signal) => {
      await syncCalendarEvents({ signal });
    },
    { maxDuration: CALENDAR_SYNC_MAX_DURATION },
  );

  useMountEffect(() => {
    if (!manager) return;

    let timeoutId: number | undefined;
    const clearNextSync = () => {
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId);
        timeoutId = undefined;
      }
    };
    const scheduleNextSync = () => {
      clearNextSync();
      timeoutId = window.setTimeout(() => {
        timeoutId = undefined;
        scheduleCalendarSync(manager);
      }, CALENDAR_SYNC_INTERVAL);
    };
    const taskRunListenerId = manager.addTaskRunRunningListener(
      CALENDAR_SYNC_TASK_ID,
      null,
      (_manager, _taskId, _taskRunId, running) => {
        if (running === undefined) {
          scheduleNextSync();
        } else {
          clearNextSync();
        }
      },
    );
    const unlisten = appleCalendarEvents.calendarChangedEvent.listen(() => {
      scheduleCalendarSync(manager);
    });
    scheduleCalendarSync(manager);

    return () => {
      clearNextSync();
      manager.delListener(taskRunListenerId);
      unlisten.then((fn) => fn());
    };
  });

  useRegisterTask(
    EVENT_NOTIFICATION_TASK_ID,
    async () => {
      await checkEventNotifications(
        notificationEvent && !notificationsDisabled,
        notifiedEventsRef.current,
      );
    },
    {
      maxDuration: EVENT_NOTIFICATION_MAX_DURATION,
      maxRetries: REPEATING_TASK_MAX_RETRIES,
      retryDelay: EVENT_NOTIFICATION_INTERVAL,
    },
  );

  useScheduleTaskRun(EVENT_NOTIFICATION_TASK_ID, undefined, 0, {
    repeatDelay: EVENT_NOTIFICATION_INTERVAL,
  });

  useRegisterTask(
    AUDIO_RETENTION_TASK_ID,
    async () => {
      const deletedSessionIds = await cleanupExpiredAudio(audioRetention);
      for (const sessionId of deletedSessionIds) {
        void queryClient.invalidateQueries({
          queryKey: ["audio", sessionId, "exist"],
        });
        void queryClient.invalidateQueries({
          queryKey: ["audio", sessionId, "url"],
        });
      }
      void cleanupExpiredVoiceprintCandidates();
    },
    {
      maxDuration: AUDIO_RETENTION_MAX_DURATION,
      maxRetries: REPEATING_TASK_MAX_RETRIES,
      retryDelay: AUDIO_RETENTION_INTERVAL,
    },
  );

  useScheduleTaskRun(AUDIO_RETENTION_TASK_ID, undefined, 0, {
    repeatDelay: AUDIO_RETENTION_INTERVAL,
  });

  return null;
}
