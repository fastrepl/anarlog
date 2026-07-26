import { useCallback, useMemo } from "react";
import { create } from "zustand";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import {
  LEGACY_MAIN_VALUES_ID,
  LEGACY_SETTINGS_ID,
} from "~/settings/legacy-snapshots";

type IgnoredEvent = { tracking_id: string; last_seen: string };
type IgnoredRecurringSeries = { id: string; last_seen: string };
type AppSettingSqlRow = { id?: string; value_json: string | null };

const IGNORED_EVENTS_ID = "ignored_events";
const IGNORED_SERIES_ID = "ignored_recurring_series";

// Optimistic overlay for ignore/unignore: an override forces its desired
// state until its own write settles, so rapid toggles keep showing the
// latest intent even while earlier writes are still landing. Tokens let a
// newer toggle supersede an older override's cleanup.
type IgnoredOverride = { ignored: boolean; token: number };

let overrideToken = 0;

// After a write settles, the live query needs a beat to re-emit the new base
// before the override is dropped — clearing instantly would flash the stale
// base state.
const OVERRIDE_SETTLE_GRACE_MS = 1000;

const useIgnoredOverrides = create<{
  events: Record<string, IgnoredOverride>;
  series: Record<string, IgnoredOverride>;
  set: (
    kind: "events" | "series",
    id: string,
    override: IgnoredOverride,
  ) => void;
  clear: (kind: "events" | "series", id: string) => void;
}>((set) => ({
  events: {},
  series: {},
  set: (kind, id, override) =>
    set((state) => ({ [kind]: { ...state[kind], [id]: override } })),
  clear: (kind, id) =>
    set((state) => {
      const { [id]: _, ...rest } = state[kind];
      return { [kind]: rest };
    }),
}));

function applyOverrides(
  baseIds: Set<string>,
  overrides: Record<string, IgnoredOverride>,
): Set<string> {
  let result = baseIds;
  for (const [id, override] of Object.entries(overrides)) {
    if (baseIds.has(id) === override.ignored) continue;
    if (result === baseIds) result = new Set(baseIds);
    if (override.ignored) {
      result.add(id);
    } else {
      result.delete(id);
    }
  }
  return result;
}

function runWithIgnoredOverride(
  kind: "events" | "series",
  id: string,
  ignored: boolean,
  write: () => Promise<void>,
  errorMessage: string,
) {
  const token = ++overrideToken;
  useIgnoredOverrides.getState().set(kind, id, { ignored, token });
  const clearIfCurrent = () => {
    const current = useIgnoredOverrides.getState()[kind][id];
    if (current?.token === token) {
      useIgnoredOverrides.getState().clear(kind, id);
    }
  };
  write().then(
    () => setTimeout(clearIfCurrent, OVERRIDE_SETTLE_GRACE_MS),
    (error: unknown) => {
      console.error(errorMessage, error);
      clearIfCurrent();
    },
  );
}

export function useIgnoredEvents() {
  const ignoredEvents = useSettingList<IgnoredEvent>(IGNORED_EVENTS_ID);
  const ignoredSeries =
    useSettingList<IgnoredRecurringSeries>(IGNORED_SERIES_ID);
  const eventOverrides = useIgnoredOverrides((state) => state.events);
  const seriesOverrides = useIgnoredOverrides((state) => state.series);
  const baseIgnoredIds = useMemo(
    () => new Set(ignoredEvents.map((event) => event.tracking_id)),
    [ignoredEvents],
  );
  const baseIgnoredSeriesIds = useMemo(
    () => new Set(ignoredSeries.map((series) => series.id)),
    [ignoredSeries],
  );
  const ignoredIds = useMemo(
    () => applyOverrides(baseIgnoredIds, eventOverrides),
    [baseIgnoredIds, eventOverrides],
  );
  const ignoredSeriesIds = useMemo(
    () => applyOverrides(baseIgnoredSeriesIds, seriesOverrides),
    [baseIgnoredSeriesIds, seriesOverrides],
  );

  const isIgnored = useCallback(
    (
      trackingId: string | null | undefined,
      recurrenceSeriesId: string | null | undefined,
    ) =>
      Boolean(
        trackingId &&
        (ignoredIds.has(trackingId) ||
          (recurrenceSeriesId && ignoredSeriesIds.has(recurrenceSeriesId))),
      ),
    [ignoredIds, ignoredSeriesIds],
  );
  const ignoreEvent = useCallback((trackingId: string) => {
    runWithIgnoredOverride(
      "events",
      trackingId,
      true,
      () =>
        mutateSettingList<IgnoredEvent>(IGNORED_EVENTS_ID, (events) => [
          ...events.filter((event) => event.tracking_id !== trackingId),
          { tracking_id: trackingId, last_seen: new Date().toISOString() },
        ]),
      "[calendar] failed to ignore event",
    );
  }, []);
  const unignoreEvent = useCallback((trackingId: string) => {
    runWithIgnoredOverride(
      "events",
      trackingId,
      false,
      () =>
        mutateSettingList<IgnoredEvent>(IGNORED_EVENTS_ID, (events) =>
          events.filter((event) => event.tracking_id !== trackingId),
        ),
      "[calendar] failed to unignore event",
    );
  }, []);
  const ignoreSeries = useCallback((seriesId: string) => {
    runWithIgnoredOverride(
      "series",
      seriesId,
      true,
      () =>
        mutateSettingList<IgnoredRecurringSeries>(
          IGNORED_SERIES_ID,
          (series) => [
            ...series.filter((entry) => entry.id !== seriesId),
            { id: seriesId, last_seen: new Date().toISOString() },
          ],
        ),
      "[calendar] failed to ignore series",
    );
  }, []);
  const unignoreSeries = useCallback((seriesId: string) => {
    runWithIgnoredOverride(
      "series",
      seriesId,
      false,
      () =>
        mutateSettingList<IgnoredRecurringSeries>(IGNORED_SERIES_ID, (series) =>
          series.filter((entry) => entry.id !== seriesId),
        ),
      "[calendar] failed to unignore series",
    );
  }, []);

  return {
    isIgnored,
    ignoreEvent,
    unignoreEvent,
    ignoreSeries,
    unignoreSeries,
  };
}

export async function getIgnoredEventSets(): Promise<{
  ignoredIds: Set<string>;
  ignoredSeriesIds: Set<string>;
}> {
  const rows = await liveQueryClient.execute<AppSettingSqlRow>(
    `
      SELECT id, value_json
      FROM app_settings
      WHERE id IN (?, ?, ?, ?)
    `,
    [
      IGNORED_EVENTS_ID,
      IGNORED_SERIES_ID,
      LEGACY_MAIN_VALUES_ID,
      LEGACY_SETTINGS_ID,
    ],
  );
  const events = resolveSettingList<IgnoredEvent>(rows, IGNORED_EVENTS_ID);
  const series = resolveSettingList<IgnoredRecurringSeries>(
    rows,
    IGNORED_SERIES_ID,
  );
  return {
    ignoredIds: new Set(events.map((event) => event.tracking_id)),
    ignoredSeriesIds: new Set(series.map((entry) => entry.id)),
  };
}

function useSettingList<T>(id: string): T[] {
  const { data = EMPTY_LIST } = useLiveQuery<AppSettingSqlRow, T[]>({
    sql: `
      SELECT COALESCE(
        (SELECT value_json FROM app_settings WHERE id = ?),
        (SELECT
          CASE
            WHEN json_valid(value_json) THEN json_extract(value_json, ?)
            ELSE NULL
          END
        FROM app_settings
        WHERE id = ?),
        (SELECT
          CASE
            WHEN json_valid(value_json) THEN json_extract(value_json, ?)
            ELSE NULL
          END
        FROM app_settings
        WHERE id = ?)
      ) AS value_json
    `,
    params: [
      id,
      `$.${id}`,
      LEGACY_MAIN_VALUES_ID,
      `$.${id}`,
      LEGACY_SETTINGS_ID,
    ],
    mapRows: (rows) => parseSettingList<T>(rows[0]?.value_json),
  });
  return data;
}

async function mutateSettingList<T>(
  id: string,
  mutation: (items: T[]) => T[],
): Promise<void> {
  return enqueueDatabaseWrite(`app-setting:${id}`, async () => {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const rows = await liveQueryClient.execute<AppSettingSqlRow>(
        `
          SELECT id, value_json
          FROM app_settings
          WHERE id IN (?, ?, ?)
        `,
        [id, LEGACY_MAIN_VALUES_ID, LEGACY_SETTINGS_ID],
      );
      const direct = rows.find((row) => row.id === id);
      const current = resolveSettingList<T>(rows, id);
      const nextJson = JSON.stringify(mutation(current));
      const now = new Date().toISOString();
      const [updated = 0] = await executeTransaction([
        direct
          ? {
              sql: `
                UPDATE app_settings
                SET value_json = ?, updated_at = ?
                WHERE id = ? AND value_json = ?
              `,
              params: [nextJson, now, id, direct.value_json],
            }
          : {
              sql: `
                INSERT INTO app_settings (id, value_json, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(id) DO NOTHING
              `,
              params: [id, nextJson, now],
            },
      ]);

      if (updated === 1) return;
    }

    throw new Error(`Setting ${id} changed too frequently`);
  });
}

function resolveSettingList<T>(rows: AppSettingSqlRow[], id: string): T[] {
  const direct = rows.find((row) => row.id === id);
  if (direct) return parseSettingList<T>(direct.value_json);
  const legacyMain = rows.find((row) => row.id === LEGACY_MAIN_VALUES_ID);
  if (hasLegacySetting(legacyMain?.value_json, id)) {
    return parseLegacySettingList<T>(legacyMain?.value_json, id);
  }
  const legacySettings = rows.find((row) => row.id === LEGACY_SETTINGS_ID);
  return parseLegacySettingList<T>(legacySettings?.value_json, id);
}

function hasLegacySetting(
  value: string | null | undefined,
  id: string,
): boolean {
  if (!value) return false;
  try {
    const document = JSON.parse(value);
    return (
      document !== null &&
      typeof document === "object" &&
      Object.prototype.hasOwnProperty.call(document, id)
    );
  } catch {
    return false;
  }
}

function parseLegacySettingList<T>(
  value: string | null | undefined,
  id: string,
): T[] {
  if (!value) return [];
  try {
    const document = JSON.parse(value) as Record<string, unknown>;
    const nested = document[id];
    return parseSettingList<T>(
      typeof nested === "string" ? nested : JSON.stringify(nested ?? []),
    );
  } catch {
    return [];
  }
}

function parseSettingList<T>(value: string | null | undefined): T[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) return parsed as T[];
    if (typeof parsed === "string") {
      const nested = JSON.parse(parsed);
      return Array.isArray(nested) ? (nested as T[]) : [];
    }
  } catch {
    return [];
  }
  return [];
}

const EMPTY_LIST: never[] = [];
