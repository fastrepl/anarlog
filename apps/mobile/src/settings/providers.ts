import * as SecureStore from "expo-secure-store";

import { supabase } from "@/auth/client";
import { execute, executeTransaction } from "@/db";

import {
  defaultProviderConfig,
  providerStorageKey,
  validateProviderConfig,
  type ProviderConfig,
  type ProviderKind,
} from "./providers-model";

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

export async function saveProviderConfig(
  accountId: string | null,
  kind: ProviderKind,
  config: ProviderConfig,
  apiKey?: string,
): Promise<void> {
  const normalized = validateProviderConfig(kind, config);
  if (normalized.provider !== "anarlog") {
    const key =
      apiKey?.trim() ||
      (await readProviderKey(accountId, kind, normalized.provider));
    if (!key) throw new Error("Enter an API key for this provider.");
    if (key.length > 8192 || /[\r\n]/.test(key))
      throw new Error("Enter a valid API key.");
    if (apiKey?.trim())
      await SecureStore.setItemAsync(
        providerStorageKey(accountId, kind, normalized.provider),
        key,
        secureOptions,
      );
  }
  await executeTransaction([
    {
      sql: `INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at`,
      params: [
        providerStorageKey(accountId, kind),
        JSON.stringify(normalized),
        new Date().toISOString(),
      ],
    },
  ]);
}

export async function removeProviderKey(
  accountId: string | null,
  kind: ProviderKind,
  provider: string,
): Promise<void> {
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
  if (config.provider === "anarlog") {
    if (session?.access_token)
      return { ...config, apiKey: session.access_token };
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
