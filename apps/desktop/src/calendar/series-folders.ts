import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { normalizeFolderPath } from "~/session/folders";

export type SeriesFolderRule = {
  series_id: string;
  folder_path: string;
};

type AppSettingSqlRow = { value_json: string | null };

const SERIES_FOLDERS_ID = "auto_add_series_folders";
const DECLINED_SERIES_ID = "auto_add_series_folders_declined";

export function useSeriesFolderRules(): SeriesFolderRule[] {
  const { data = EMPTY_RULES } = useLiveQuery<
    AppSettingSqlRow,
    SeriesFolderRule[]
  >({
    sql: `SELECT value_json FROM app_settings WHERE id = ?`,
    params: [SERIES_FOLDERS_ID],
    mapRows: (rows) => parseSeriesFolderRules(rows[0]?.value_json),
  });
  return data;
}

export function useDeclinedSeriesFolderIds(): string[] {
  const { data = EMPTY_DECLINED } = useLiveQuery<AppSettingSqlRow, string[]>({
    sql: `SELECT value_json FROM app_settings WHERE id = ?`,
    params: [DECLINED_SERIES_ID],
    mapRows: (rows) => parseDeclinedSeriesIds(rows[0]?.value_json),
  });
  return data;
}

export async function getSeriesFolderRule(
  seriesId: string,
): Promise<string | null> {
  if (!seriesId) return null;
  const rules = await loadSeriesFolderRules();
  return ruleFolderForSeries(rules, seriesId);
}

export function ruleFolderForSeries(
  rules: SeriesFolderRule[],
  seriesId: string | null | undefined,
): string | null {
  if (!seriesId) return null;
  return rules.find((rule) => rule.series_id === seriesId)?.folder_path ?? null;
}

export async function setSeriesFolderRule(
  seriesId: string,
  folderPath: string,
): Promise<void> {
  const normalized = normalizeFolderPath(folderPath);
  if (!seriesId || !normalized) return;
  await enqueueDatabaseWrite(`app-setting:${SERIES_FOLDERS_ID}`, async () => {
    const rules = await loadSeriesFolderRules();
    const next = rules.filter((rule) => rule.series_id !== seriesId);
    next.push({ series_id: seriesId, folder_path: normalized });
    await writeSeriesFolderRules(next);

    const declined = await loadDeclinedSeriesIds();
    if (declined.includes(seriesId)) {
      await writeDeclinedSeriesIds(declined.filter((id) => id !== seriesId));
    }
  });
}

export async function declineSeriesFolderRule(seriesId: string): Promise<void> {
  if (!seriesId) return;
  await enqueueDatabaseWrite(`app-setting:${SERIES_FOLDERS_ID}`, async () => {
    const declined = await loadDeclinedSeriesIds();
    if (!declined.includes(seriesId)) {
      await writeDeclinedSeriesIds([...declined, seriesId]);
    }
  });
}

export async function clearSeriesFolderRule(seriesId: string): Promise<void> {
  if (!seriesId) return;
  await enqueueDatabaseWrite(`app-setting:${SERIES_FOLDERS_ID}`, async () => {
    const rules = await loadSeriesFolderRules();
    const next = rules.filter((rule) => rule.series_id !== seriesId);
    if (next.length !== rules.length) {
      await writeSeriesFolderRules(next);
    }
  });
}

export async function clearSeriesFolderRulesForFolder(
  folderPath: string,
): Promise<void> {
  const normalized = normalizeFolderPath(folderPath);
  if (!normalized) return;
  await enqueueDatabaseWrite(`app-setting:${SERIES_FOLDERS_ID}`, async () => {
    const rules = await loadSeriesFolderRules();
    const next = rules.filter(
      (rule) =>
        rule.folder_path !== normalized &&
        !rule.folder_path.startsWith(`${normalized}/`),
    );
    if (next.length !== rules.length) {
      await writeSeriesFolderRules(next);
    }
  });
}

export async function remapSeriesFolderRules(
  oldPath: string,
  newPath: string,
): Promise<void> {
  if (!oldPath || !newPath || oldPath === newPath) return;
  await enqueueDatabaseWrite(`app-setting:${SERIES_FOLDERS_ID}`, async () => {
    const rules = await loadSeriesFolderRules();
    let changed = false;
    const next = rules.map((rule) => {
      const remapped = remapRulePath(rule.folder_path, oldPath, newPath);
      if (remapped !== rule.folder_path) {
        changed = true;
        return { ...rule, folder_path: remapped };
      }
      return rule;
    });
    if (changed) {
      await writeSeriesFolderRules(next);
    }
  });
}

export function parseSeriesFolderRules(
  value: string | null | undefined,
): SeriesFolderRule[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((entry) => {
      if (typeof entry !== "object" || entry === null) return [];
      const { series_id, folder_path } = entry as Record<string, unknown>;
      if (typeof series_id !== "string" || !series_id) return [];
      const normalized = normalizeFolderPath(
        typeof folder_path === "string" ? folder_path : "",
      );
      if (!normalized) return [];
      return [{ series_id, folder_path: normalized }];
    });
  } catch {
    return [];
  }
}

function parseDeclinedSeriesIds(value: string | null | undefined): string[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is string => typeof entry === "string" && entry !== "",
    );
  } catch {
    return [];
  }
}

async function loadSeriesFolderRules(): Promise<SeriesFolderRule[]> {
  const rows = await liveQueryClient.execute<AppSettingSqlRow>(
    `SELECT value_json FROM app_settings WHERE id = ?`,
    [SERIES_FOLDERS_ID],
  );
  return parseSeriesFolderRules(rows[0]?.value_json);
}

async function loadDeclinedSeriesIds(): Promise<string[]> {
  const rows = await liveQueryClient.execute<AppSettingSqlRow>(
    `SELECT value_json FROM app_settings WHERE id = ?`,
    [DECLINED_SERIES_ID],
  );
  return parseDeclinedSeriesIds(rows[0]?.value_json);
}

async function writeSeriesFolderRules(
  rules: SeriesFolderRule[],
): Promise<void> {
  await writeSetting(SERIES_FOLDERS_ID, JSON.stringify(rules));
}

async function writeDeclinedSeriesIds(seriesIds: string[]): Promise<void> {
  await writeSetting(DECLINED_SERIES_ID, JSON.stringify(seriesIds));
}

async function writeSetting(id: string, valueJson: string): Promise<void> {
  await executeTransaction([
    {
      sql: `
        INSERT INTO app_settings (id, value_json, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          value_json = excluded.value_json,
          updated_at = excluded.updated_at
      `,
      params: [id, valueJson, new Date().toISOString()],
    },
  ]);
}

function remapRulePath(path: string, oldPath: string, newPath: string): string {
  if (path === oldPath) return newPath;
  if (path.startsWith(`${oldPath}/`))
    return newPath + path.slice(oldPath.length);
  return path;
}

const EMPTY_RULES: SeriesFolderRule[] = [];
const EMPTY_DECLINED: string[] = [];
