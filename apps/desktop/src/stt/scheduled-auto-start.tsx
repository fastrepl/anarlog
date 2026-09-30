import { useCallback, useRef } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
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
import { hasScheduledAutoStartInFlight } from "~/stt/scheduled-auto-start-state";
import { decideAutomaticMeetingAttendance } from "~/stt/scheduled-meeting-attendance";

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
    recurrence_series_id,
    attendance_json
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
  attendance_json: string | null;
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

    if (
      !decideAutomaticMeetingAttendance({
        attendanceJson: row.attendance_json,
        nowMs,
      }).eligible
    ) {
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

async function readDueScheduledMeeting(
  eventId: string,
): Promise<ScheduledMeetingRow | null> {
  const [row] = await liveQueryClient.execute<ScheduledMeetingRow>(
    `
      SELECT
        id,
        started_at,
        meeting_link,
        tracking_id_event,
        recurrence_series_id,
        attendance_json
      FROM events
      WHERE id = ?
        AND deleted_at IS NULL
        AND is_all_day = 0
        AND started_at <> ''
        AND meeting_link <> ''
      LIMIT 1
    `,
    [eventId],
  );

  return (
    selectDueMeetings({
      rows: row ? [row] : [],
      nowMs: Date.now(),
      firedEventIds: new Set(),
    })[0] ?? null
  );
}

export async function readDueScheduledSessionMeeting(
  sessionId: string,
): Promise<ScheduledMeetingRow | null> {
  const [row] = await liveQueryClient.execute<ScheduledMeetingRow>(
    `
      SELECT
        events.id,
        events.started_at,
        events.meeting_link,
        events.tracking_id_event,
        events.recurrence_series_id,
        events.attendance_json
      FROM sessions
      JOIN events ON events.id = sessions.event_id
      WHERE sessions.id = ?
        AND sessions.deleted_at IS NULL
        AND events.deleted_at IS NULL
        AND events.is_all_day = 0
        AND events.started_at <> ''
        AND events.meeting_link <> ''
      LIMIT 1
    `,
    [sessionId],
  );

  return (
    selectDueMeetings({
      rows: row ? [row] : [],
      nowMs: Date.now(),
      firedEventIds: new Set(),
    })[0] ?? null
  );
}

function isIgnoredScheduledMeeting(
  row: ScheduledMeetingRow,
  ignoredIds: ReadonlySet<string>,
  ignoredSeriesIds: ReadonlySet<string>,
): boolean {
  return (
    ignoredIds.has(row.tracking_id_event) ||
    (Boolean(row.recurrence_series_id) &&
      ignoredSeriesIds.has(row.recurrence_series_id))
  );
}

export async function startScheduledMeeting(
  row: ScheduledMeetingRow,
  autoJoin: boolean,
): Promise<"started" | "ignored" | "blocked" | "ineligible"> {
  if (listenerStore.getState().live.status === "active") {
    return "ignored";
  }

  const { ignoredIds, ignoredSeriesIds } = await getIgnoredEventSets();
  if (
    listenerStore.getState().live.status === "active" ||
    isIgnoredScheduledMeeting(row, ignoredIds, ignoredSeriesIds)
  ) {
    return "ignored";
  }

  let currentRow = await readDueScheduledMeeting(row.id);
  if (!currentRow) {
    return "ineligible";
  }

  const sessionId = await getOrCreateSessionForEventId(currentRow.id);
  if (listenerStore.getState().live.status === "active") {
    return "ignored";
  }
  if (!listenerStore.getState().canStartLiveSession(sessionId)) {
    return "blocked";
  }

  // Session creation can cross a calendar sync boundary. Re-check immediately
  // before opening a URL or arming capture so a last-second decline,
  // cancellation, or reschedule cannot trigger either automatic action.
  currentRow = await readDueScheduledMeeting(currentRow.id);
  if (!currentRow) {
    return "ineligible";
  }

  const latestIgnoredEvents = await getIgnoredEventSets();
  if (
    listenerStore.getState().live.status === "active" ||
    isIgnoredScheduledMeeting(
      currentRow,
      latestIgnoredEvents.ignoredIds,
      latestIgnoredEvents.ignoredSeriesIds,
    )
  ) {
    return "ignored";
  }

  // Joining and listening are independent: the link opens as soon as the
  // meeting is due, while listening still has to wait for the session tab,
  // the STT connection, and capture readiness (and may be abandoned).
  if (autoJoin) {
    void openerCommands.openUrl(currentRow.meeting_link, null);
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
    const temporarilyIneligibleEventIds = new Set<string>();

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
      // The live query can deliver rows after cleanup (StrictMode remounts the
      // effect before the subscription resolves); a torn-down instance must
      // not arm a deadline and start the same meeting a second time.
      if (cancelled) {
        return;
      }

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
      }

      const due = selectDueMeetings({
        rows,
        nowMs: Date.now(),
        firedEventIds,
      }).filter((row) => !temporarilyIneligibleEventIds.has(row.id));
      const next = due[0];
      const scheduleAfterTransientBlock = () => {
        if (next) scheduleTick(TICK_MS);
        else scheduleNextStart();
      };
      const liveStatus = listenerStore.getState().live.status;
      const action = getScheduledAutoStartAction(liveStatus);
      if (action === "skip") {
        // Do not let an overlapping meeting start after the active recording ends.
        for (const row of due) {
          firedEventIds.add(row.id);
        }
        scheduleNextStart();
        return;
      }

      if (
        action === "retry" ||
        hasScheduledAutoStartInFlight() ||
        hasPendingAutoStart(useTabs.getState().tabs)
      ) {
        scheduleAfterTransientBlock();
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

          // Keep this event dormant until calendar data changes, then try the
          // next overlapping meeting immediately instead of letting the newer
          // ineligible event hide it for the full grace window.
          if (outcome === "ineligible") {
            temporarilyIneligibleEventIds.add(next.id);
            scheduleTick(1);
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
          if (cancelled) return;
          temporarilyIneligibleEventIds.clear();
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
