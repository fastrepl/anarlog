import { commands as openerCommands } from "@anlg/plugin-opener2";
import { getCurrentWebviewWindowLabel } from "@anlg/plugin-windows";
import { safeParseDate } from "@anlg/utils";

import { getIgnoredEventSets } from "~/calendar/ignored-events";
import { liveQueryClient } from "~/db";
import { getOrCreateSessionForEventId } from "~/session/queries";
import { useConfigValues } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { listenerStore } from "~/store/zustand/listener/instance";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { hasScheduledAutoStartInFlight } from "~/stt/scheduled-auto-start-state";

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

    const startedAt = safeParseDate(row.started_at);
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

  if (autoJoin) {
    void openerCommands.openUrl(row.meeting_link, null);
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

  useMountEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    let cancelled = false;
    let rows: ScheduledMeetingRow[] = [];
    let unsubscribe: (() => Promise<void>) | null = null;
    let starting = false;
    const firedEventIds = new Set<string>();

    const tick = () => {
      // Ticks come from both the interval and calendar updates; without this a
      // second tick could open another meeting mid-start.
      if (starting) {
        return;
      }

      if (!autoStartRef.current) {
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

      if (
        hasScheduledAutoStartInFlight() ||
        hasPendingAutoStart(useTabs.getState().tabs)
      ) {
        return;
      }

      if (listenerStore.getState().live.status !== "inactive") {
        return;
      }

      const due = selectDueMeetings({
        rows,
        nowMs: Date.now(),
        firedEventIds,
      });
      const next = due[0];
      if (!next) {
        return;
      }

      starting = true;
      void startScheduledMeeting(next, Boolean(autoJoinRef.current))
        .then((outcome) => {
          // A blocked start is transient (another session finalizing, a start
          // already in flight), so leave it eligible for the next tick.
          if (outcome === "blocked") {
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
        })
        .finally(() => {
          starting = false;
        });
    };

    const interval = setInterval(tick, TICK_MS);

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
      clearInterval(interval);
      void unsubscribe?.();
    };
  });

  return null;
}
