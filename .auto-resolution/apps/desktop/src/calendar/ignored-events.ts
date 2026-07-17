import { useCallback, useMemo } from "react";

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

export function useIgnoredEvents() {
  const ignoredEvents = useSettingList<IgnoredEvent>(IGNORED_EVENTS_ID);
  const ignoredSeries =
    useSettingList<IgnoredRecurringSeries>(IGNORED_SERIES_ID);
  const ignoredIds = useMemo(
    () => new Set(ignoredEvents.map((event) => event.tracking_id)),
    [ignoredEvents],
  );
  const ignoredSeriesIds = useMemo(
    () => new Set(ignoredSeries.map((series) => series.id)),
    [ignoredSeries],
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
    void mutateSettingList<IgnoredEvent>(IGNORED_EVENTS_ID, (events) => [
      ...events.filter((event) => event.tracking_id !== trackingId),
      { tracking_id: trackingId, last_seen: new Date().toISOString() },
    ]).catch((error) => {
      console.error("[calendar] failed to ignore event", error);
    });
  }, []);
  const unignoreEvent = useCallback((trackingId: string) => {
    void mutateSettingList<IgnoredEvent>(IGNORED_EVENTS_ID, (events) =>
      events.filter((event) => event.tracking_id !== trackingId),
    ).catch((error) => {
      console.error("[calendar] failed to unignore event", error);
    });
  }, []);
  const ignoreSeries = useCallback((seriesId: string) => {
    void mutateSettingList<IgnoredRecurringSeries>(
      IGNORED_SERIES_ID,
      (series) => [
        ...series.filter((entry) => entry.id !== seriesId),
        { id: seriesId, last_seen: new Date().toISOString() },
      ],
    ).catch((error) => {
      console.error("[calendar] failed to ignore series", error);
    });
  }, []);
  const unignoreSeries = useCallback((seriesId: string) => {
    void mutateSettingList<IgnoredRecurringSeries>(
      IGNORED_SERIES_ID,
      (series) => series.filter((entry) => entry.id !== seriesId),
    ).catch((error) => {
      console.error("[calendar] failed to unignore series", error);
    });
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
