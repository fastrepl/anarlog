import { hasSummaryContent } from "@anlg/utils/session";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { normalizeFolderPath } from "~/session/folders";

export type FolderDefaultAccess = "participants" | "workspace";

export type FolderDefaultAccessRule = {
  folder_path: string;
  access: FolderDefaultAccess;
  workspace_id: string;
  since: string;
};

export type FolderShareCandidate = {
  sessionId: string;
  folderPath: string;
  createdAt: string;
  documentId: string;
  documentUpdatedAt: string;
  title: string;
  body: string;
};

type AppSettingSqlRow = { value_json: string | null };

const RULES_ID = "folder_default_share_access";
const APPLIED_ID = "folder_default_share_access_applied";
const EMPTY_RULES: FolderDefaultAccessRule[] = [];

export function useFolderDefaultAccessRules(): FolderDefaultAccessRule[] {
  const { data = EMPTY_RULES } = useLiveQuery<
    AppSettingSqlRow,
    FolderDefaultAccessRule[]
  >({
    sql: `SELECT value_json FROM app_settings WHERE id = ?`,
    params: [RULES_ID],
    mapRows: (rows) => parseFolderDefaultAccessRules(rows[0]?.value_json),
  });
  return data;
}

export function folderDefaultAccessRule(
  rules: FolderDefaultAccessRule[],
  folderPath: string,
): FolderDefaultAccessRule | null {
  const normalized = normalizeFolderPath(folderPath);
  if (!normalized) return null;
  return rules.find((rule) => rule.folder_path === normalized) ?? null;
}

export async function setFolderDefaultAccess(
  folderPath: string,
  value: { access: FolderDefaultAccess; workspaceId: string } | null,
  now = new Date().toISOString(),
): Promise<void> {
  const normalized = normalizeFolderPath(folderPath);
  if (!normalized) return;
  await enqueueDatabaseWrite(`app-setting:${RULES_ID}`, async () => {
    const rules = await loadRules();
    const existing = folderDefaultAccessRule(rules, normalized);
    const next = rules.filter((rule) => rule.folder_path !== normalized);
    if (value) {
      const workspaceId = value.access === "workspace" ? value.workspaceId : "";
      const unchanged =
        existing?.access === value.access &&
        existing.workspace_id === workspaceId;
      next.push({
        folder_path: normalized,
        access: value.access,
        workspace_id: workspaceId,
        since: (unchanged && existing.since) || now,
      });
    }
    await writeSetting(RULES_ID, JSON.stringify(next));
  });
}

export async function remapFolderDefaultAccessRules(
  oldPath: string,
  newPath: string,
): Promise<void> {
  if (!oldPath || !newPath || oldPath === newPath) return;
  await enqueueDatabaseWrite(`app-setting:${RULES_ID}`, async () => {
    const rules = await loadRules();
    let changed = false;
    const next = rules.map((rule) => {
      const remapped = remapPath(rule.folder_path, oldPath, newPath);
      if (remapped === rule.folder_path) return rule;
      changed = true;
      return { ...rule, folder_path: remapped };
    });
    if (changed) await writeSetting(RULES_ID, JSON.stringify(next));
  });
}

export async function clearFolderDefaultAccessRulesForFolder(
  folderPath: string,
): Promise<void> {
  const normalized = normalizeFolderPath(folderPath);
  if (!normalized) return;
  await enqueueDatabaseWrite(`app-setting:${RULES_ID}`, async () => {
    const rules = await loadRules();
    const next = rules.filter(
      (rule) =>
        rule.folder_path !== normalized &&
        !rule.folder_path.startsWith(`${normalized}/`),
    );
    if (next.length !== rules.length) {
      await writeSetting(RULES_ID, JSON.stringify(next));
    }
  });
}

export function folderShareCandidateReady(
  candidate: FolderShareCandidate,
  rule: FolderDefaultAccessRule,
  summaryGenerating: boolean,
): boolean {
  if (summaryGenerating || candidate.createdAt < rule.since) return false;
  return hasSummaryContent(candidate.body, candidate.title);
}

export async function loadFolderDefaultAccessAppliedSessionIds(): Promise<
  Set<string>
> {
  const rows = await liveQueryClient.execute<AppSettingSqlRow>(
    `SELECT value_json FROM app_settings WHERE id = ?`,
    [APPLIED_ID],
  );
  return new Set(parseStringArray(rows[0]?.value_json));
}

export async function markFolderDefaultAccessApplied(
  sessionId: string,
): Promise<void> {
  if (!sessionId) return;
  await enqueueDatabaseWrite(`app-setting:${APPLIED_ID}`, async () => {
    const applied = await loadFolderDefaultAccessAppliedSessionIds();
    if (applied.has(sessionId)) return;
    applied.add(sessionId);
    await writeSetting(APPLIED_ID, JSON.stringify([...applied]));
  });
}

export function parseFolderDefaultAccessRules(
  value: string | null | undefined,
): FolderDefaultAccessRule[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap<FolderDefaultAccessRule>((entry) => {
      if (typeof entry !== "object" || entry === null) return [];
      const { folder_path, access, workspace_id, since } = entry as Record<
        string,
        unknown
      >;
      const normalized = normalizeFolderPath(
        typeof folder_path === "string" ? folder_path : "",
      );
      if (!normalized || typeof since !== "string" || !since) return [];
      if (access === "participants") {
        return [{ folder_path: normalized, access, workspace_id: "", since }];
      }
      if (
        access === "workspace" &&
        typeof workspace_id === "string" &&
        workspace_id
      ) {
        return [{ folder_path: normalized, access, workspace_id, since }];
      }
      return [];
    });
  } catch {
    return [];
  }
}

function parseStringArray(value: string | null | undefined): string[] {
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

function remapPath(path: string, oldPath: string, newPath: string): string {
  if (path === oldPath) return newPath;
  if (path.startsWith(`${oldPath}/`)) {
    return `${newPath}${path.slice(oldPath.length)}`;
  }
  return path;
}

async function loadRules(): Promise<FolderDefaultAccessRule[]> {
  const rows = await liveQueryClient.execute<AppSettingSqlRow>(
    `SELECT value_json FROM app_settings WHERE id = ?`,
    [RULES_ID],
  );
  return parseFolderDefaultAccessRules(rows[0]?.value_json);
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
