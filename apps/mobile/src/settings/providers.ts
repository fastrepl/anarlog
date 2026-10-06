import * as SecureStore from "expo-secure-store";
import { fetch } from "expo/fetch";

import { verifyProviderCredentials } from "@anlg/provider-validation";

import {
  decodeJwtPayload,
  deriveBillingInfo,
  ProRequiredError,
} from "@/auth/billing";
import { supabase } from "@/auth/client";
import { execute, executeTransaction } from "@/db";

import {
  defaultProviderConfig,
  isOAuthSubscriptionProvider,
  providerStorageKey,
  validateProviderApiKey,
  validateProviderConfig,
  validateProviderConnection,
  type ProviderConfig,
  type ProviderKind,
} from "./providers-model";
import {
  removeSubscriptionCredential,
  resolveSubscriptionCredential,
} from "./subscription-access";
import { listSubscriptionModels } from "./subscription-oauth";

const secureOptions: SecureStore.SecureStoreOptions = {
  keychainService: "so.anarlog.mobile.providers",
  keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
};

export async function readProviderConfig(
  accountId: string | null,
  kind: ProviderKind,
): Promise<ProviderConfig> {
  const rows = await execute<{ value_json: string }>(
    "SELECT value_json FROM app_settings WHERE id = ?",
    [providerStorageKey(accountId, kind)],
  );
  if (!rows[0]) return defaultProviderConfig(kind);
  const config = JSON.parse(rows[0].value_json) as ProviderConfig;
  return validateProviderConfig(kind, config);
}

export async function readProviderKey(
  accountId: string | null,
  kind: ProviderKind,
  provider: string,
): Promise<string | null> {
  if (provider === "anarlog") return null;
  return SecureStore.getItemAsync(
    providerStorageKey(accountId, kind, provider),
    secureOptions,
  );
}

export async function readProviderSetup(
  accountId: string | null,
  kind: ProviderKind,
  provider: string,
): Promise<ProviderConfig> {
  const savedId = providerStorageKey(accountId, kind, provider);
  const rows = await execute<{ value_json: string }>(
    "SELECT value_json FROM app_settings WHERE id IN (?, ?) ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END",
    [
      savedId,
      providerStorageKey(accountId, kind),
      providerStorageKey(accountId, kind),
    ],
  );
  for (const row of rows) {
    const config = JSON.parse(row.value_json) as ProviderConfig;
    if (config.provider === provider)
      return config.model
        ? validateProviderConfig(kind, config)
        : { ...validateProviderConnection(kind, config), model: "" };
  }
  return defaultProviderConfig(kind, provider);
}

export async function readProviderStatus(
  accountId: string | null,
  kind: ProviderKind,
  provider: string,
) {
  if (isOAuthSubscriptionProvider(provider)) {
    const config = await readProviderSetup(accountId, kind, provider);
    let hasKey = false;
    try {
      hasKey = Boolean(await readProviderKey(accountId, kind, provider));
      if (!hasKey) return { config, hasKey, isConfigured: false };
      const credential = await resolveSubscriptionCredential(
        accountId,
        provider,
      );
      const models = await listSubscriptionModels(provider, credential, fetch);
      if (!models.length)
        throw new Error(
          "No subscription models are available for this account.",
        );
      return { config, hasKey, isConfigured: true };
    } catch (error) {
      return {
        config,
        hasKey,
        isConfigured: false,
        verificationError:
          error instanceof Error
            ? error.message
            : "Reconnect your subscription in Settings.",
      };
    }
  }
  const [config, apiKey] = await Promise.all([
    readProviderSetup(accountId, kind, provider),
    readProviderKey(accountId, kind, provider),
  ]);
  let hasKey = false;
  let isConfigured = provider === "anarlog";
  let verificationError: string | undefined;
  try {
    validateProviderApiKey(apiKey ?? "");
    hasKey = true;
    validateProviderConnection(kind, config);
    await verifyProviderCredentials({ ...config, apiKey: apiKey! }, fetch);
    isConfigured = true;
  } catch (error) {
    if (hasKey)
      verificationError =
        error instanceof Error ? error.message : "Couldn’t verify this key.";
  }
  return { config, hasKey, isConfigured, verificationError };
}

export async function saveProviderConfig(
  accountId: string | null,
  kind: ProviderKind,
  config: ProviderConfig,
  apiKey?: string,
): Promise<void> {
  return persistProviderConfig(accountId, kind, config, apiKey, true);
}

export async function saveProviderSetup(
  accountId: string | null,
  kind: ProviderKind,
  config: ProviderConfig,
  apiKey?: string,
): Promise<void> {
  return persistProviderConfig(accountId, kind, config, apiKey, false);
}

export async function saveProviderConnection(
  accountId: string | null,
  kind: ProviderKind,
  connection: { provider: string; baseUrl: string },
  apiKey?: string,
): Promise<void> {
  const normalized = validateProviderConnection(kind, connection);
  const saved = await readProviderSetup(accountId, kind, connection.provider);
  return persistProviderConfig(
    accountId,
    kind,
    { ...saved, ...normalized },
    apiKey,
    false,
    true,
  );
}

async function persistProviderConfig(
  accountId: string | null,
  kind: ProviderKind,
  config: ProviderConfig,
  apiKey: string | undefined,
  activate: boolean,
  connectionOnly = false,
): Promise<void> {
  const normalized = connectionOnly
    ? { ...config, ...validateProviderConnection(kind, config) }
    : validateProviderConfig(kind, config);
  if (isOAuthSubscriptionProvider(normalized.provider)) {
    await resolveSubscriptionCredential(accountId, normalized.provider);
  } else if (normalized.provider !== "anarlog") {
    const key = validateProviderApiKey(
      apiKey?.trim() ||
        (await readProviderKey(accountId, kind, normalized.provider)) ||
        "",
    );
    await verifyProviderCredentials({ ...normalized, apiKey: key }, fetch);
    if (apiKey?.trim())
      await SecureStore.setItemAsync(
        providerStorageKey(accountId, kind, normalized.provider),
        key,
        secureOptions,
      );
  }
  const activeId = providerStorageKey(accountId, kind);
  const value = JSON.stringify(normalized);
  const updatedAt = new Date().toISOString();
  await executeTransaction([
    {
      // Preserve settings from builds that stored only the active provider.
      sql: `INSERT INTO app_settings (id, value_json, updated_at)
      SELECT id || '.' || json_extract(value_json, '$.provider'), value_json, updated_at
      FROM app_settings WHERE id = ?
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at`,
      params: [activeId],
    },
    {
      sql: `INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at`,
      params: [
        providerStorageKey(accountId, kind, normalized.provider),
        value,
        updatedAt,
      ],
    },
    activate
      ? {
          sql: `INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at`,
          params: [activeId, value, updatedAt],
        }
      : {
          sql: `UPDATE app_settings SET value_json = ?, updated_at = ?
      WHERE id = ? AND json_extract(value_json, '$.provider') = ?`,
          params: [value, updatedAt, activeId, normalized.provider],
        },
  ]);
}

export async function removeProviderKey(
  accountId: string | null,
  kind: ProviderKind,
  provider: string,
): Promise<void> {
  if (isOAuthSubscriptionProvider(provider))
    return removeSubscriptionCredential(accountId, provider);
  await SecureStore.deleteItemAsync(
    providerStorageKey(accountId, kind, provider),
    secureOptions,
  );
}

export async function resolveProvider(kind: ProviderKind) {
  const auth = await supabase?.auth?.getSession();
  if (auth?.error) throw new Error("Sign in again to continue.");
  const session = auth?.data.session;
  const config = await readProviderConfig(session?.user.id ?? null, kind);
  if (isOAuthSubscriptionProvider(config.provider)) {
    const credential = await resolveSubscriptionCredential(
      session?.user.id ?? null,
      config.provider,
    );
    return {
      ...config,
      apiKey: credential.access,
      accountId: credential.accountId,
    };
  }
  if (config.provider === "anarlog") {
    if (session?.access_token) {
      if (!deriveBillingInfo(decodeJwtPayload(session.access_token)).isPro)
        throw new ProRequiredError();
      return { ...config, apiKey: session.access_token };
    }
    if (!supabase) return { ...config, apiKey: "" };
    throw new Error("Sign in to use Anarlog Pro.");
  }
  const key = await readProviderKey(
    session?.user.id ?? null,
    kind,
    config.provider,
  );
  if (!key) throw new Error("Add an API key in Settings to use this provider.");
  return { ...config, apiKey: key };
}
