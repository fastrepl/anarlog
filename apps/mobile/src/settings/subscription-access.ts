import { randomUUID } from "expo-crypto";
import * as SecureStore from "expo-secure-store";
import { fetch } from "expo/fetch";

import { providerStorageKey } from "./providers-model";
import {
  parseSubscriptionCredential,
  refreshSubscriptionCredential,
  type SubscriptionCredential,
  type OAuthSubscriptionProvider,
} from "./subscription-oauth";

const options: SecureStore.SecureStoreOptions = {
  keychainService: "so.anarlog.mobile.providers",
  keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
};
const pending = new Map<string, Promise<unknown>>();

function serialized<T>(
  account: string | null,
  provider: OAuthSubscriptionProvider,
  operation: (key: string) => Promise<T>,
): Promise<T> {
  const key = providerStorageKey(account, "llm", provider);
  const previous = pending.get(key) ?? Promise.resolve();
  const next = previous.catch(() => {}).then(() => operation(key));
  pending.set(key, next);
  void next
    .finally(() => {
      if (pending.get(key) === next) pending.delete(key);
    })
    .catch(() => {});
  return next;
}

async function manifest(
  key: string,
): Promise<{ version: string; count: number } | null> {
  const raw = await SecureStore.getItemAsync(key, options);
  if (!raw) return null;
  try {
    const value = JSON.parse(raw);
    if (
      typeof value.version === "string" &&
      /^[a-zA-Z0-9-]+$/.test(value.version) &&
      Number.isInteger(value.count) &&
      value.count > 0 &&
      value.count <= 64
    )
      return value;
  } catch {}
  throw new Error(
    "Reconnect your subscription in Settings to repair the saved connection.",
  );
}

async function read(
  key: string,
  provider: OAuthSubscriptionProvider,
): Promise<SubscriptionCredential | null> {
  const saved = await manifest(key);
  if (!saved) return null;
  const chunks = await Promise.all(
    Array.from({ length: saved.count }, (_, index) =>
      SecureStore.getItemAsync(`${key}.${saved.version}.${index}`, options),
    ),
  );
  const credential = chunks.every((chunk) => chunk !== null)
    ? parseSubscriptionCredential(provider, chunks.join(""))
    : null;
  if (!credential)
    throw new Error(
      "Reconnect your subscription in Settings to repair the saved connection.",
    );
  return credential;
}

async function cleanup(key: string, saved: { version: string; count: number }) {
  await Promise.allSettled(
    Array.from({ length: saved.count }, (_, index) =>
      SecureStore.deleteItemAsync(`${key}.${saved.version}.${index}`, options),
    ),
  );
}

async function write(
  key: string,
  credential: SubscriptionCredential,
  provider: OAuthSubscriptionProvider,
  signal?: AbortSignal,
) {
  const value = JSON.stringify(credential);
  if (!parseSubscriptionCredential(provider, value))
    throw new Error("Invalid subscription connection.");
  const previous = await manifest(key).catch(() => null);
  // Subscription JWTs exceed some iOS keychain value limits. Publish a new manifest
  // only after every chunk is saved, keeping the old connection on write failure.
  const next = { version: randomUUID(), count: Math.ceil(value.length / 1500) };
  if (next.count > 64)
    throw new Error("Subscription connection is too large to save.");
  try {
    for (let index = 0; index < next.count; index++) {
      signal?.throwIfAborted();
      await SecureStore.setItemAsync(
        `${key}.${next.version}.${index}`,
        value.slice(index * 1500, (index + 1) * 1500),
        options,
      );
    }
    signal?.throwIfAborted();
    await SecureStore.setItemAsync(key, JSON.stringify(next), options);
  } catch (error) {
    await cleanup(key, next);
    throw error;
  }
  if (previous) await cleanup(key, previous);
}

export function readSubscriptionCredential(
  account: string | null,
  provider: OAuthSubscriptionProvider,
) {
  return serialized(account, provider, (key) => read(key, provider));
}

export function saveSubscriptionCredential(
  account: string | null,
  provider: OAuthSubscriptionProvider,
  credential: SubscriptionCredential,
  signal?: AbortSignal,
) {
  return serialized(account, provider, (key) =>
    write(key, credential, provider, signal),
  );
}

export function removeSubscriptionCredential(
  account: string | null,
  provider: OAuthSubscriptionProvider,
) {
  return serialized(account, provider, async (key) => {
    const saved = await manifest(key).catch(() => null);
    await SecureStore.deleteItemAsync(key, options);
    if (saved) await cleanup(key, saved);
  });
}

export function resolveSubscriptionCredential(
  account: string | null,
  provider: OAuthSubscriptionProvider,
) {
  return serialized(account, provider, async (key) => {
    const saved = await read(key, provider);
    if (!saved)
      throw new Error(
        `Connect ${provider === "chatgpt" ? "ChatGPT" : "your subscription"} in Settings to use your subscription.`,
      );
    if (saved.access && saved.expires - 120_000 > Date.now()) return saved;
    const next = await refreshSubscriptionCredential(provider, saved, fetch);
    await write(key, next, provider);
    return next;
  });
}
