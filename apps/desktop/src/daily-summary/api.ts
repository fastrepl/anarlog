import { useMemo } from "react";

import { dailySummaries, eq, and } from "@hypr/db";
import { commands as activityCaptureCommands } from "@hypr/plugin-activity-capture";

import { db, useDrizzleLiveQuery, useLiveQuery } from "~/db";

export type DailyActivityAppStat = {
  appName: string;
  count: number;
};

export type DailyActivityStats = {
  observationCount: number;
  screenshotCount: number;
  analysisCount: number;
  uniqueAppCount: number;
  firstObservationAtMs: number | null;
  lastObservationAtMs: number | null;
  topApps: DailyActivityAppStat[];
};

export type DailyObservationAnalysis = {
  capturedAtMs: number;
  observationId: string;
  screenshotId: string;
  screenshotKind: string;
  appName: string;
  windowTitle: string | null;
  summary: string;
};

export type DailySummaryTopic = {
  title: string;
  summary: string;
};

export type DailySummaryTimelineItem = {
  time: string;
  summary: string;
};

export type StoredDailySummary = {
  id: string;
  date: string;
  content: string;
  timeline: DailySummaryTimelineItem[];
  topics: DailySummaryTopic[];
  status: string;
  sourceCursorMs: number;
  sourceFingerprint: string;
  generatedAt: string;
  generationError: string;
  updatedAt: string;
};

export type DailySummarySnapshot = {
  stats: DailyActivityStats;
  analyses: DailyObservationAnalysis[];
  summary: StoredDailySummary | null;
  sourceCursorMs: number;
  sourceFingerprint: string;
};

type ObservationAnalysisRow = {
  capturedAtMs: number;
  observationId: string;
  screenshotId: string;
  screenshotKind: string;
  appName: string;
  windowTitle: string | null;
  summary: string;
};

type DailySummaryRow = {
  id: string;
  date: string;
  content: string;
  timeline_json: string;
  topics_json: string;
  status: string;
  source_cursor_ms: number;
  source_fingerprint: string;
  generated_at: string;
  generation_error: string;
  updated_at: string;
};

type DailyStatsRow = {
  observationCount: number;
  screenshotCount: number;
  analysisCount: number;
  uniqueAppCount: number;
  firstObservationAtMs: number | null;
  lastObservationAtMs: number | null;
  topAppsJson: string;
  eventCursorMs: number;
  analysisCursorMs: number;
};

const ANALYSES_SQL = `WITH ranked AS (
  SELECT
    observation_id AS observationId,
    screenshot_id AS screenshotId,
    screenshot_kind AS screenshotKind,
    captured_at_ms AS capturedAtMs,
    app_name AS appName,
    NULLIF(window_title, '') AS windowTitle,
    summary,
    ROW_NUMBER() OVER (
      PARTITION BY observation_id
      ORDER BY CASE screenshot_kind
        WHEN 'settled' THEN 0
        WHEN 'refresh' THEN 1
        ELSE 2
      END,
      captured_at_ms DESC,
      id DESC
    ) AS rankInObservation
  FROM activity_observation_analyses
  WHERE captured_at_ms >= ? AND captured_at_ms < ?
)
SELECT
  observationId,
  screenshotId,
  screenshotKind,
  capturedAtMs,
  appName,
  windowTitle,
  summary
FROM ranked
WHERE rankInObservation = 1
ORDER BY capturedAtMs ASC`;

const STATS_SQL = `WITH started_events AS (
  SELECT occurred_at_ms, app_name
  FROM activity_observation_events
  WHERE event_kind = 'started' AND occurred_at_ms >= ? AND occurred_at_ms < ?
),
ranked_analyses AS (
  SELECT
    observation_id,
    captured_at_ms,
    ROW_NUMBER() OVER (
      PARTITION BY observation_id
      ORDER BY CASE screenshot_kind
        WHEN 'settled' THEN 0
        WHEN 'refresh' THEN 1
        ELSE 2
      END,
      captured_at_ms DESC,
      id DESC
    ) AS rankInObservation
  FROM activity_observation_analyses
  WHERE captured_at_ms >= ? AND captured_at_ms < ?
),
preferred_analyses AS (
  SELECT captured_at_ms
  FROM ranked_analyses
  WHERE rankInObservation = 1
),
top_apps AS (
  SELECT app_name AS appName, COUNT(*) AS count
  FROM started_events
  WHERE app_name != ''
  GROUP BY app_name
  ORDER BY count DESC, app_name ASC
  LIMIT 5
)
SELECT
  COALESCE((SELECT COUNT(*) FROM started_events), 0) AS observationCount,
  COALESCE((
    SELECT COUNT(*)
    FROM activity_screenshots
    WHERE captured_at_ms >= ? AND captured_at_ms < ?
  ), 0) AS screenshotCount,
  COALESCE((SELECT COUNT(*) FROM preferred_analyses), 0) AS analysisCount,
  COALESCE((
    SELECT COUNT(DISTINCT app_name)
    FROM started_events
    WHERE app_name != ''
  ), 0) AS uniqueAppCount,
  (SELECT MIN(occurred_at_ms) FROM started_events) AS firstObservationAtMs,
  (SELECT MAX(occurred_at_ms) FROM started_events) AS lastObservationAtMs,
  COALESCE((
    SELECT json_group_array(json_object('appName', appName, 'count', count))
    FROM top_apps
  ), '[]') AS topAppsJson,
  COALESCE((SELECT MAX(occurred_at_ms) FROM started_events), 0) AS eventCursorMs,
  COALESCE((SELECT MAX(captured_at_ms) FROM preferred_analyses), 0) AS analysisCursorMs`;

function summaryQuery(date: string, dailyNoteId: string) {
  return db
    .select()
    .from(dailySummaries)
    .where(
      and(
        eq(dailySummaries.date, date),
        eq(dailySummaries.dailyNoteId, dailyNoteId),
      ),
    )
    .limit(1);
}

function parseJsonArray<T>(value: string, fallback: T[]) {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? (parsed as T[]) : fallback;
  } catch {
    return fallback;
  }
}

function mapAnalyses(
  rows: ObservationAnalysisRow[],
): DailyObservationAnalysis[] {
  return rows.map((row) => ({
    capturedAtMs: row.capturedAtMs,
    observationId: row.observationId,
    screenshotId: row.screenshotId,
    screenshotKind: row.screenshotKind,
    appName: row.appName,
    windowTitle: row.windowTitle,
    summary: row.summary,
  }));
}

function mapSummary(rows: DailySummaryRow[]): StoredDailySummary | null {
  const [row] = rows;
  if (!row) {
    return null;
  }

  const timelineRaw = row.timeline_json;
  const topicsRaw = row.topics_json;

  return {
    id: row.id,
    date: row.date,
    content: row.content,
    timeline:
      typeof timelineRaw === "string"
        ? parseJsonArray<DailySummaryTimelineItem>(timelineRaw, [])
        : Array.isArray(timelineRaw)
          ? (timelineRaw as DailySummaryTimelineItem[])
          : [],
    topics:
      typeof topicsRaw === "string"
        ? parseJsonArray<DailySummaryTopic>(topicsRaw, [])
        : Array.isArray(topicsRaw)
          ? (topicsRaw as DailySummaryTopic[])
          : [],
    status: row.status,
    sourceCursorMs: row.source_cursor_ms,
    sourceFingerprint: row.source_fingerprint,
    generatedAt: row.generated_at,
    generationError: row.generation_error,
    updatedAt: row.updated_at,
  };
}

function mapStats(rows: DailyStatsRow[]): {
  stats: DailyActivityStats;
  sourceCursorMs: number;
} {
  const row = rows[0] ?? {
    observationCount: 0,
    screenshotCount: 0,
    analysisCount: 0,
    uniqueAppCount: 0,
    firstObservationAtMs: null,
    lastObservationAtMs: null,
    topAppsJson: "[]",
    eventCursorMs: 0,
    analysisCursorMs: 0,
  };

  return {
    stats: {
      observationCount: row.observationCount,
      screenshotCount: row.screenshotCount,
      analysisCount: row.analysisCount,
      uniqueAppCount: row.uniqueAppCount,
      firstObservationAtMs: row.firstObservationAtMs,
      lastObservationAtMs: row.lastObservationAtMs,
      topApps: parseJsonArray<DailyActivityAppStat>(row.topAppsJson, []),
    },
    sourceCursorMs: Math.max(row.eventCursorMs, row.analysisCursorMs),
  };
}

function dailyNoteId(date: string) {
  return `daily-note-${date}`;
}

function buildSourceFingerprint(params: {
  observationCount: number;
  screenshotCount: number;
  analysisCount: number;
  sourceCursorMs: number;
}) {
  return `observations:${params.observationCount}|screenshots:${params.screenshotCount}|analyses:${params.analysisCount}|cursor:${params.sourceCursorMs}`;
}

export function useDailySummarySnapshot(params: {
  date: string;
  startMs: number;
  endMs: number;
}) {
  const analysesQuery = useLiveQuery<
    ObservationAnalysisRow,
    DailyObservationAnalysis[]
  >({
    sql: ANALYSES_SQL,
    params: [params.startMs, params.endMs],
    mapRows: mapAnalyses,
  });
  const statsQuery = useLiveQuery<
    DailyStatsRow,
    { stats: DailyActivityStats; sourceCursorMs: number }
  >({
    sql: STATS_SQL,
    params: [
      params.startMs,
      params.endMs,
      params.startMs,
      params.endMs,
      params.startMs,
      params.endMs,
    ],
    mapRows: mapStats,
  });
  const summaryResult = useDrizzleLiveQuery<
    DailySummaryRow,
    StoredDailySummary | null
  >(summaryQuery(params.date, dailyNoteId(params.date)), {
    mapRows: mapSummary,
  });

  const data = useMemo<DailySummarySnapshot | undefined>(() => {
    if (
      !analysesQuery.data ||
      !statsQuery.data ||
      summaryResult.data === undefined
    ) {
      return undefined;
    }

    return {
      stats: statsQuery.data.stats,
      analyses: analysesQuery.data,
      summary: summaryResult.data,
      sourceCursorMs: statsQuery.data.sourceCursorMs,
      sourceFingerprint: buildSourceFingerprint({
        observationCount: statsQuery.data.stats.observationCount,
        screenshotCount: statsQuery.data.stats.screenshotCount,
        analysisCount: statsQuery.data.stats.analysisCount,
        sourceCursorMs: statsQuery.data.sourceCursorMs,
      }),
    };
  }, [analysesQuery.data, statsQuery.data, summaryResult.data]);

  return {
    data,
    isLoading:
      analysesQuery.isLoading ||
      statsQuery.isLoading ||
      summaryResult.isLoading,
    error: analysesQuery.error ?? statsQuery.error ?? summaryResult.error,
  };
}

export async function saveDailySummary(params: {
  date: string;
  content: string;
  timeline: DailySummaryTimelineItem[];
  topics: DailySummaryTopic[];
  sourceCursorMs: number;
  sourceFingerprint: string;
  generatedAt: string;
}): Promise<StoredDailySummary> {
  const result = await activityCaptureCommands.saveDailySummary(params);

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}
