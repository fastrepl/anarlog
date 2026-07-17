import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands as store2Commands } from "@hypr/plugin-store2";

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
const PROVIDER_SECRET_SCOPE = "ai-provider-api-keys";
const MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX =
  "macOS couldn't access your login Keychain.";
const EMPTY_PROVIDER_API_KEYS: Record<string, string> = {};
const EMPTY_ROWS: AppSettingRow[] = [];

export function useAiProviders(
  type: AiProviderType,
): Record<string, AiProviderConfig> {
  return useAiProvidersState(type).providers;
}

export function useAiProvidersState(type: AiProviderType): {
  providers: Record<string, AiProviderConfig>;
  isReady: boolean;
} {
  const { data: rows = EMPTY_ROWS, isLoading } = useLiveQuery<
    AppSettingRow,
    AppSettingRow[]
  >({
    sql: `SELECT id, value_json FROM app_settings ORDER BY id`,
    mapRows: (rows) => rows,
  });
  const providers = parseAiProviders(rows, type);
  const providerIds = Object.keys(providers).sort();
  const secureApiKeysQuery = useQuery({
    queryKey: ["ai-provider-api-keys", type, providerIds],
    queryFn: () => loadSecureAiProviderApiKeys(providerIds, type),
    enabled: !isLoading,
    staleTime: Infinity,
  });
  const secureApiKeys = secureApiKeysQuery.data ?? EMPTY_PROVIDER_API_KEYS;

  return {
    providers: Object.fromEntries(
      Object.entries(providers).map(([rowId, provider]) => [
        rowId,
        {
          ...provider,
          api_key: secureApiKeys[rowId] ?? provider.api_key,
        },
      ]),
    ),
    isReady: !isLoading && secureApiKeysQuery.data !== undefined,
  };
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
    const previousApiKey = await getProviderApiKey(type, providerId);

    try {
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
          api_key: changes.api_key ?? previousApiKey ?? current?.api_key ?? "",
        };
        await setProviderApiKey(type, providerId, next.api_key);

        const persisted = { ...next, api_key: "" };
        const now = new Date().toISOString();
        const statements = [
          direct
            ? {
                sql: `
                UPDATE app_settings
                SET value_json = ?, updated_at = ?
                WHERE id = ? AND value_json = ?
              `,
                params: [
                  JSON.stringify(persisted),
                  now,
                  storageId,
                  direct.value_json,
                ],
              }
            : {
                sql: `
                INSERT INTO app_settings (id, value_json, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(id) DO NOTHING
              `,
                params: [storageId, JSON.stringify(persisted), now],
              },
        ];

        const legacyRow = rows.find((row) => row.id === LEGACY_SETTINGS_ID);
        const redactedLegacy = redactLegacyProviderApiKey(
          legacyRow?.value_json,
          type,
          providerId,
        );
        if (legacyRow && redactedLegacy) {
          statements.push({
            sql: `
            UPDATE app_settings
            SET value_json = ?, updated_at = ?
            WHERE id = ?
          `,
            params: [redactedLegacy, now, LEGACY_SETTINGS_ID],
          });
        }

        const [updated = 0] = await executeTransaction(statements);
        if (updated === 1) return;
      }

      throw new Error(`Provider ${type}:${providerId} changed too frequently`);
    } catch (error) {
      await setProviderApiKey(type, providerId, previousApiKey ?? "");
      throw error;
    }
  });
}

export function useSetAiProvider(type: AiProviderType, providerId: string) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ["set-ai-provider", type, providerId],
    mutationFn: (
      changes: Partial<Pick<AiProviderConfig, "base_url" | "api_key">>,
    ) => setAiProvider(type, providerId, changes),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["ai-provider-api-keys", type],
        }),
        queryClient.invalidateQueries({
          queryKey: ["default-ai-selection", type],
        }),
      ]);
    },
  });
}

export function isKeychainAccessError(error: unknown): boolean {
  return (
    error instanceof Error &&
    error.message.startsWith(MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX)
  );
}

export async function repairKeychainAccess(): Promise<void> {
  const result = await store2Commands.repairKeychainAccess();
  if (result.status === "error") {
    throw new Error(result.error);
  }
}

export async function loadSecureAiProviderApiKeys(
  providerRowIds: string[],
  type: AiProviderType,
): Promise<Record<string, string>> {
  const apiKeys: Record<string, string> = {};

  for (const rowId of providerRowIds) {
    const providerId = rowId.slice(`${type}:`.length);
    const apiKey = await getProviderApiKey(type, providerId);
    if (apiKey) {
      apiKeys[rowId] = apiKey;
    }
  }

  return apiKeys;
}

export async function migratePlaintextAiProviderApiKeys(): Promise<void> {
  let rows = await liveQueryClient.execute<AppSettingRow>(
    `SELECT id, value_json FROM app_settings ORDER BY id`,
  );
  const migratedLlm = await migratePlaintextProviderApiKeys(rows, "llm");
  if (migratedLlm) {
    rows = await liveQueryClient.execute<AppSettingRow>(
      `SELECT id, value_json FROM app_settings ORDER BY id`,
    );
  }
  const migratedStt = await migratePlaintextProviderApiKeys(rows, "stt");
  if (migratedLlm || migratedStt) {
    await liveQueryClient.execute(`PRAGMA wal_checkpoint(TRUNCATE)`);
  }
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

async function getProviderApiKey(
  type: AiProviderType,
  providerId: string,
): Promise<string | null> {
  const result = await store2Commands.getSecret(
    PROVIDER_SECRET_SCOPE,
    providerRowId(type, providerId),
  );
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

async function setProviderApiKey(
  type: AiProviderType,
  providerId: string,
  apiKey: string,
): Promise<void> {
  const key = providerRowId(type, providerId);
  const result = apiKey
    ? await store2Commands.setSecret(PROVIDER_SECRET_SCOPE, key, apiKey)
    : await store2Commands.deleteSecret(PROVIDER_SECRET_SCOPE, key);
  if (result.status === "error") {
    throw new Error(result.error);
  }
}

async function migratePlaintextProviderApiKeys(
  rows: AppSettingRow[],
  type: AiProviderType,
): Promise<boolean> {
  const legacyRow = rows.find((row) => row.id === LEGACY_SETTINGS_ID);
  const legacyProviders = parseAiProviders(legacyRow ? [legacyRow] : [], type);
  const directProviders = parseAiProviders(
    rows.filter((row) => row.id.startsWith(providerStorageId(type, ""))),
    type,
  );
  const rowIds = new Set([
    ...Object.keys(legacyProviders),
    ...Object.keys(directProviders),
  ]);
  const providerIds: string[] = [];

  for (const rowId of rowIds) {
    const providerId = rowId.slice(`${type}:`.length);
    const directApiKey = directProviders[rowId]?.api_key.trim() ?? "";
    const legacyApiKey = legacyProviders[rowId]?.api_key.trim() ?? "";
    if (!directApiKey && !legacyApiKey) continue;

    const existingApiKey = await getProviderApiKey(type, providerId);
    if (!existingApiKey) {
      await setProviderApiKey(type, providerId, directApiKey || legacyApiKey);
    }
    providerIds.push(providerId);
  }

  if (providerIds.length > 0) {
    await redactPlaintextProviderApiKeys(rows, type, providerIds);
  }

  return providerIds.length > 0;
}

async function redactPlaintextProviderApiKeys(
  rows: AppSettingRow[],
  type: AiProviderType,
  providerIds: string[],
): Promise<void> {
  const migrated = new Set(providerIds);
  const now = new Date().toISOString();
  const statements = rows.flatMap((row) => {
    const prefix = providerStorageId(type, "");
    if (!row.id.startsWith(prefix)) {
      return [];
    }

    const providerId = row.id.slice(prefix.length);
    const config = parseProviderValue(row.value_json, type);
    if (!migrated.has(providerId) || !config?.api_key) {
      return [];
    }

    return [
      {
        sql: `
          UPDATE app_settings
          SET value_json = ?, updated_at = ?
          WHERE id = ? AND value_json = ?
        `,
        params: [
          JSON.stringify({ ...config, api_key: "" }),
          now,
          row.id,
          row.value_json,
        ],
      },
    ];
  });

  const legacyRow = rows.find((row) => row.id === LEGACY_SETTINGS_ID);
  if (legacyRow) {
    let redactedLegacy = legacyRow.value_json;
    for (const providerId of providerIds) {
      redactedLegacy =
        redactLegacyProviderApiKey(redactedLegacy, type, providerId) ??
        redactedLegacy;
    }
    if (redactedLegacy !== legacyRow.value_json) {
      statements.push({
        sql: `
          UPDATE app_settings
          SET value_json = ?, updated_at = ?
          WHERE id = ?
        `,
        params: [redactedLegacy, now, LEGACY_SETTINGS_ID],
      });
    }
  }

  if (statements.length > 0) {
    await executeTransaction([
      { sql: `PRAGMA secure_delete = ON`, params: [] },
      ...statements,
    ]);
  }
}

function redactLegacyProviderApiKey(
  valueJson: string | undefined,
  type: AiProviderType,
  providerId: string,
): string | null {
  if (!valueJson) return null;

  try {
    const document = JSON.parse(valueJson) as Record<string, unknown>;
    const ai = parseObjectValue(document.ai);
    const providers = parseObjectValue(ai[type]);
    const provider = parseObjectValue(providers[providerId]);
    if (typeof provider.api_key !== "string" || !provider.api_key) {
      return null;
    }

    provider.api_key = "";
    return JSON.stringify(document);
  } catch {
    return null;
  }
}

function providerStorageId(type: AiProviderType, providerId: string): string {
  return `ai_provider:${type}:${providerId}`;
}

function providerRowId(type: AiProviderType, providerId: string): string {
  return `${type}:${providerId}`;
}
