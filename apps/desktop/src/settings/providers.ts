import { useCallback } from "react";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

export type AiProviderType = "llm" | "stt";

export type AiProviderConfig = {
  type: AiProviderType;
  base_url: string;
  api_key: string;
};

type AppSettingRow = { id: string; value_json: string };

const LEGACY_SETTINGS_ID = "legacy_settings_document";
const EMPTY_PROVIDERS: Record<string, AiProviderConfig> = {};

export function useAiProviders(
  type: AiProviderType,
): Record<string, AiProviderConfig> {
  const { data = EMPTY_PROVIDERS } = useLiveQuery<
    AppSettingRow,
    Record<string, AiProviderConfig>
  >({
    sql: `SELECT id, value_json FROM app_settings ORDER BY id`,
    mapRows: (rows) => parseAiProviders(rows, type),
  });
  return data;
}

export function useAiProvider(
  type: AiProviderType,
  providerId: string | null | undefined,
): AiProviderConfig | undefined {
  const providers = useAiProviders(type);
  return providerId ? providers[providerRowId(type, providerId)] : undefined;
}

export function setAiProvider(
  type: AiProviderType,
  providerId: string,
  changes: Partial<Pick<AiProviderConfig, "base_url" | "api_key">>,
): Promise<void> {
  const storageId = providerStorageId(type, providerId);
  return enqueueDatabaseWrite(storageId, async () => {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const rows = await liveQueryClient.execute<AppSettingRow>(
        `
          SELECT id, value_json
          FROM app_settings
          WHERE id IN (?, ?)
        `,
        [storageId, LEGACY_SETTINGS_ID],
      );
      const direct = rows.find((row) => row.id === storageId);
      const legacy = parseLegacyProvider(
        rows.find((row) => row.id === LEGACY_SETTINGS_ID)?.value_json,
        type,
        providerId,
      );
      const current = direct
        ? (parseProviderValue(direct.value_json, type) ?? legacy)
        : legacy;
      const next: AiProviderConfig = {
        type,
        base_url: changes.base_url ?? current?.base_url ?? "",
        api_key: changes.api_key ?? current?.api_key ?? "",
      };
      const now = new Date().toISOString();
      const [updated = 0] = await executeTransaction([
        direct
          ? {
              sql: `
                UPDATE app_settings
                SET value_json = ?, updated_at = ?
                WHERE id = ? AND value_json = ?
              `,
              params: [JSON.stringify(next), now, storageId, direct.value_json],
            }
          : {
              sql: `
                INSERT INTO app_settings (id, value_json, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(id) DO NOTHING
              `,
              params: [storageId, JSON.stringify(next), now],
            },
      ]);
      if (updated === 1) return;
    }

    throw new Error(`Provider ${type}:${providerId} changed too frequently`);
  });
}

export function useSetAiProvider(type: AiProviderType, providerId: string) {
  return useCallback(
    (changes: Partial<Pick<AiProviderConfig, "base_url" | "api_key">>) => {
      void setAiProvider(type, providerId, changes).catch((error) => {
        console.error(
          `[settings] failed to update provider ${type}:${providerId}`,
          error,
        );
      });
    },
    [providerId, type],
  );
}

export function parseAiProviders(
  rows: AppSettingRow[],
  type: AiProviderType,
): Record<string, AiProviderConfig> {
  const result: Record<string, AiProviderConfig> = {};
  const legacy = rows.find((row) => row.id === LEGACY_SETTINGS_ID);
  const legacyDocument = parseJsonObject(legacy?.value_json);
  const legacyAi = parseObjectValue(legacyDocument.ai);
  const legacyProviders = parseObjectValue(legacyAi[type]);

  for (const [providerId, value] of Object.entries(legacyProviders)) {
    const config = normalizeProvider(value, type);
    if (config) result[providerRowId(type, providerId)] = config;
  }

  const prefix = providerStorageId(type, "");
  for (const row of rows) {
    if (!row.id.startsWith(prefix)) continue;
    const providerId = row.id.slice(prefix.length);
    if (!providerId) continue;
    const config = parseProviderValue(row.value_json, type);
    if (config) result[providerRowId(type, providerId)] = config;
  }

  return result;
}

function parseLegacyProvider(
  valueJson: string | undefined,
  type: AiProviderType,
  providerId: string,
): AiProviderConfig | undefined {
  return parseAiProviders(
    valueJson ? [{ id: LEGACY_SETTINGS_ID, value_json: valueJson }] : [],
    type,
  )[providerRowId(type, providerId)];
}

function parseProviderValue(
  valueJson: string,
  type: AiProviderType,
): AiProviderConfig | undefined {
  try {
    return normalizeProvider(JSON.parse(valueJson), type);
  } catch {
    return undefined;
  }
}

function normalizeProvider(
  value: unknown,
  type: AiProviderType,
): AiProviderConfig | undefined {
  const row = parseObjectValue(value);
  if (Object.keys(row).length === 0) return undefined;
  return {
    type,
    base_url: typeof row.base_url === "string" ? row.base_url : "",
    api_key: typeof row.api_key === "string" ? row.api_key : "",
  };
}

function parseJsonObject(value: string | undefined): Record<string, unknown> {
  if (!value) return {};
  try {
    return parseObjectValue(JSON.parse(value));
  } catch {
    return {};
  }
}

function parseObjectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function providerStorageId(type: AiProviderType, providerId: string): string {
  return `ai_provider:${type}:${providerId}`;
}

function providerRowId(type: AiProviderType, providerId: string): string {
  return `${type}:${providerId}`;
}
