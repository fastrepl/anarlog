import { randomUUID } from "expo-crypto";
import * as SecureStore from "expo-secure-store";
import { fetch } from "expo/fetch";

import {
  parseChatgptCredential,
  refreshChatgptCredential,
  type ChatgptCredential,
} from "./chatgpt-oauth";
import { providerStorageKey } from "./providers-model";

const options: SecureStore.SecureStoreOptions = {
  keychainService: "so.anarlog.mobile.providers",
  keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
};
const pending = new Map<string, Promise<unknown>>();

function serialized<T>(
  account: string | null,
  operation: (key: string) => Promise<T>,
): Promise<T> {
  const key = providerStorageKey(account, "llm", "chatgpt");
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
    "Reconnect ChatGPT in Settings to repair the saved connection.",
  );
}

async function read(key: string): Promise<ChatgptCredential | null> {
  const saved = await manifest(key);
  if (!saved) return null;
  const chunks = await Promise.all(
    Array.from({ length: saved.count }, (_, index) =>
      SecureStore.getItemAsync(`${key}.${saved.version}.${index}`, options),
    ),
  );
  const credential = chunks.every((chunk) => chunk !== null)
    ? parseChatgptCredential(chunks.join(""))
    : null;
  if (!credential)
    throw new Error(
      "Reconnect ChatGPT in Settings to repair the saved connection.",
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
  credential: ChatgptCredential,
  signal?: AbortSignal,
) {
  const value = JSON.stringify(credential);
  if (!parseChatgptCredential(value))
    throw new Error("Invalid ChatGPT connection.");
  const previous = await manifest(key).catch(() => null);
  // ChatGPT JWTs exceed some iOS keychain value limits. Publish a new manifest
  // only after every chunk is saved, keeping the old connection on write failure.
  const next = { version: randomUUID(), count: Math.ceil(value.length / 1500) };
  if (next.count > 64)
    throw new Error("ChatGPT connection is too large to save.");
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

export function readChatgptCredential(account: string | null) {
  return serialized(account, read);
}

export function saveChatgptCredential(
  account: string | null,
  credential: ChatgptCredential,
  signal?: AbortSignal,
) {
  return serialized(account, (key) => write(key, credential, signal));
}

export function removeChatgptCredential(account: string | null) {
  return serialized(account, async (key) => {
    const saved = await manifest(key).catch(() => null);
    await SecureStore.deleteItemAsync(key, options);
    if (saved) await cleanup(key, saved);
  });
}

export function resolveChatgptCredential(account: string | null) {
  return serialized(account, async (key) => {
    const saved = await read(key);
    if (!saved)
      throw new Error("Connect ChatGPT in Settings to use your subscription.");
    if (saved.access && saved.expires - 120_000 > Date.now()) return saved;
    const next = await refreshChatgptCredential(saved, fetch);
    await write(key, next);
    return next;
  });
}
