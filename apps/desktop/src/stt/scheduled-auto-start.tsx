import { useCallback, useRef } from "react";

import { getCurrentWebviewWindowLabel } from "@anlg/plugin-windows";
import { parseEventInstant } from "@anlg/utils";

import { getIgnoredEventSets } from "~/calendar/ignored-events";
import { liveQueryClient } from "~/db";
import { getOrCreateSessionForEventId } from "~/session/queries";
import { useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import type { LiveSessionStatus } from "~/store/zustand/listener/general-shared";
import { listenerStore } from "~/store/zustand/listener/instance";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import {
  hasScheduledAutoStartInFlight,
  queueScheduledAutoJoin,
  takeScheduledAutoJoin,
} from "~/stt/scheduled-auto-start-state";

// A meeting that started while the app was asleep or quit is still worth
// recording, but only briefly — reopening hours later must not start capturing
// a meeting that is already over.
export const SCHEDULED_AUTO_START_GRACE_MS = 5 * 60_000;

const TICK_MS = 15_000;

// Calendar blocks without a meeting link ("Lunch", "Focus time") are excluded:
// auto-start watches every calendar, so anything looser would record all day.
const SCHEDULED_MEETINGS_SQL = `
  SELECT
    id,
    started_at,
    meeting_link,
    tracking_id_event,
    recurrence_series_id
  FROM events
  WHERE deleted_at IS NULL
    AND is_all_day = 0
    AND started_at <> ''
    AND meeting_link <> ''
  ORDER BY started_at, id
`;

export type ScheduledMeetingRow = {
  id: string;
  started_at: string;
  meeting_link: string;
  tracking_id_event: string;
  recurrence_series_id: string;
};

// Back-to-back meetings overlap inside the grace window; the one that just
// started is the one the user is walking into, so the newest start comes first.
export function selectDueMeetings({
  rows,
  nowMs,
  firedEventIds,
}: {
  rows: ScheduledMeetingRow[];
  nowMs: number;
  firedEventIds: ReadonlySet<string>;
}): ScheduledMeetingRow[] {
  const due: { row: ScheduledMeetingRow; startMs: number }[] = [];

  for (const row of rows) {
    if (firedEventIds.has(row.id)) {
      continue;
    }

    const startedAt = parseEventInstant(row.started_at);
    if (!startedAt) {
      continue;
    }

    const startMs = startedAt.getTime();
    const elapsedMs = nowMs - startMs;
    if (elapsedMs < 0 || elapsedMs > SCHEDULED_AUTO_START_GRACE_MS) {
      continue;
    }

    due.push({ row, startMs });
  }

  return due.sort((a, b) => b.startMs - a.startMs).map(({ row }) => row);
}

export function hasPendingAutoStart(tabs: readonly Tab[]): boolean {
  return tabs.some(
    (tab) =>
      tab.type === "sessions" && tab.active && Boolean(tab.state.autoStart),
  );
}

export function getScheduledAutoStartAction(
  status: LiveSessionStatus,
): "start" | "retry" | "skip" {
  if (status === "active") return "skip";
  if (status === "finalizing") return "retry";
  return "start";
}

async function startScheduledMeeting(
  row: ScheduledMeetingRow,
  autoJoin: boolean,
): Promise<"started" | "ignored" | "blocked"> {
  const { ignoredIds, ignoredSeriesIds } = await getIgnoredEventSets();
  if (
    ignoredIds.has(row.tracking_id_event) ||
    (row.recurrence_series_id && ignoredSeriesIds.has(row.recurrence_series_id))
  ) {
    return "ignored";
  }

  const sessionId = await getOrCreateSessionForEventId(row.id);
  if (!listenerStore.getState().canStartLiveSession(sessionId)) {
    return "blocked";
  }

  // Defer opening the meeting until listening starts. Opening it here first
  // lets the meeting app take audio devices before capture begins.
  if (autoJoin) {
    queueScheduledAutoJoin(sessionId, row.meeting_link);
  }

  useTabs.getState().openNew({
    type: "sessions",
    id: sessionId,
    state: { view: null, autoStart: true },
  });

  return "started";
}

export function ScheduledMeetingAutoStart() {
  const {
    auto_start_scheduled_meetings: autoStart,
    auto_join_scheduled_meetings: autoJoin,
  } = useConfigValues([
    "auto_start_scheduled_meetings",
    "auto_join_scheduled_meetings",
  ] as const);
  const autoStartRef = useLatestRef(autoStart);
  const autoJoinRef = useLatestRef(autoJoin);
  const configChangedRef = useRef<() => void>(() => {});
  const configChangeNodeRef = useCallback(
    (node: HTMLSpanElement | null) => {
      if (node) configChangedRef.current();
    },
    [autoJoin, autoStart],
  );

  useMountEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let cancelled = false;
    let rows: ScheduledMeetingRow[] = [];
    let unsubscribe: (() => Promise<void>) | null = null;
    let starting = false;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    const firedEventIds = new Set<string>();

    const scheduleTick = (delayMs: number) => {
      clearTimeout(timeout);
      timeout = setTimeout(
        () => {
          timeout = undefined;
          tick();
        },
        Math.max(1, delayMs),
      );
    };

    const scheduleNextStart = () => {
      clearTimeout(timeout);
      timeout = undefined;
      if (!autoStartRef.current) return;

      const now = Date.now();
      const nextStart = rows.reduce((earliest, row) => {
        if (firedEventIds.has(row.id)) return earliest;
        const start = parseEventInstant(row.started_at)?.getTime();
        return start !== undefined && start > now
          ? Math.min(earliest, start)
          : earliest;
      }, Number.POSITIVE_INFINITY);
      if (Number.isFinite(nextStart)) {
        scheduleTick(nextStart - now);
      }
    };

    const tick = () => {
      // Deadlines and state updates can coincide; without this a second tick
      // could open another meeting mid-start.
      if (starting) {
        return;
      }

      if (!autoStartRef.current) {
        scheduleNextStart();
        return;
      }

      const tabsState = useTabs.getState();
      for (const tab of tabsState.tabs) {
        if (tab.type !== "sessions" || tab.active || !tab.state.autoStart) {
          continue;
        }

        tabsState.updateSessionTabState(tab, {
          ...tab.state,
          autoStart: null,
        });
        takeScheduledAutoJoin(tab.id);
      }

      const due = selectDueMeetings({
        rows,
        nowMs: Date.now(),
        firedEventIds,
      });
      const next = due[0];
      const scheduleAfterTransientBlock = () => {
        if (next) scheduleTick(TICK_MS);
        else scheduleNextStart();
      };

      if (
        hasScheduledAutoStartInFlight() ||
        hasPendingAutoStart(useTabs.getState().tabs)
      ) {
        scheduleAfterTransientBlock();
        return;
      }

      const liveStatus = listenerStore.getState().live.status;
      const action = getScheduledAutoStartAction(liveStatus);
      if (action === "retry") {
        scheduleAfterTransientBlock();
        return;
      }

      if (action === "skip") {
        // Do not let an overlapping meeting start after the active recording ends.
        for (const row of due) {
          firedEventIds.add(row.id);
        }
        scheduleNextStart();
        return;
      }

      if (!next) {
        scheduleNextStart();
        return;
      }

      starting = true;
      void startScheduledMeeting(next, Boolean(autoJoinRef.current))
        .then((outcome) => {
          // A blocked start is transient (another session finalizing, a start
          // already in flight), so leave it eligible for the next tick.
          if (outcome === "blocked") {
            scheduleTick(TICK_MS);
            return;
          }

          if (outcome === "ignored") {
            firedEventIds.add(next.id);
            return;
          }

          // Anything overlapping the meeting we just started is one the user
          // walked out of; claiming them keeps a later tick from falling back.
          for (const row of due) {
            firedEventIds.add(row.id);
          }
        })
        .catch((error) => {
          console.error(
            "[listener] failed to auto-start scheduled meeting",
            error,
          );
          scheduleTick(TICK_MS);
        })
        .finally(() => {
          starting = false;
          if (!timeout) tick();
        });
    };
    configChangedRef.current = tick;
    const unsubscribeTabs = useTabs.subscribe(tick);
    const unsubscribeListener = listenerStore.subscribe((state, previous) => {
      if (state.live.status !== previous.live.status) tick();
    });

    void liveQueryClient
      .subscribe<ScheduledMeetingRow>(SCHEDULED_MEETINGS_SQL, [], {
        onData: (nextRows) => {
          rows = nextRows;
          tick();
        },
        onError: (error) => {
          console.error("[calendar] failed to read scheduled meetings", error);
        },
      })
      .then((stopListening) => {
        if (cancelled) {
          void stopListening();
          return;
        }
        unsubscribe = stopListening;
      })
      .catch((error) => {
        console.error(
          "[calendar] failed to subscribe to scheduled meetings",
          error,
        );
      });

    return () => {
      cancelled = true;
      configChangedRef.current = () => {};
      clearTimeout(timeout);
      unsubscribeTabs();
      unsubscribeListener();
      void unsubscribe?.();
    };
  });

  return <span ref={configChangeNodeRef} hidden aria-hidden="true" />;
}
