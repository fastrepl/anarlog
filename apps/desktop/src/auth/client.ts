import {
  createClient,
  processLock,
  type Session,
  type SupabaseClient,
  type SupportedStorage,
} from "@supabase/supabase-js";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { commands as authCommands } from "@anlg/plugin-auth";

import { env } from "~/env";

const authStorageKey = env.VITE_SUPABASE_URL
  ? `sb-${new URL(env.VITE_SUPABASE_URL).hostname.split(".")[0]}-auth-token`
  : null;

export const tauriStorage: SupportedStorage = {
  async getItem(key: string): Promise<string | null> {
    const result = await authCommands.getItem(key);
    if (result.status === "error") {
      return null;
    }
    return result.data;
  },
  async setItem(key: string, value: string): Promise<void> {
    const result = await authCommands.setItem(key, value);
    if (result.status === "error") {
      throw new Error(`auth storage setItem failed: ${result.error}`);
    }
  },
  async removeItem(key: string): Promise<void> {
    if (key === authStorageKey) {
      return;
    }

    const result = await authCommands.removeItem(key);
    if (result.status === "error") {
      throw new Error(`auth storage removeItem failed: ${result.error}`);
    }
  },
};

function parsePersistedSession(value: string | null): Session | null {
  if (!value) {
    return null;
  }

  try {
    const parsed = JSON.parse(value) as Partial<Session>;
    return typeof parsed.access_token === "string" &&
      typeof parsed.refresh_token === "string" &&
      typeof parsed.token_type === "string" &&
      typeof parsed.user?.id === "string"
      ? (parsed as Session)
      : null;
  } catch {
    return null;
  }
}

export async function readPersistedAuthSession(): Promise<Session | null> {
  if (!authStorageKey) {
    return null;
  }

  return parsePersistedSession(await tauriStorage.getItem(authStorageKey));
}

export async function persistAuthSession(session: Session): Promise<void> {
  if (!authStorageKey) {
    return;
  }

  await tauriStorage.setItem(authStorageKey, JSON.stringify(session));
}

export const supabase: SupabaseClient | null =
  env.VITE_SUPABASE_URL && env.VITE_SUPABASE_ANON_KEY
    ? createClient(env.VITE_SUPABASE_URL, env.VITE_SUPABASE_ANON_KEY, {
        global: {
          fetch: tauriFetch,
        },
        auth: {
          storage: tauriStorage,
          storageKey: authStorageKey ?? undefined,
          autoRefreshToken: true,
          persistSession: true,
          detectSessionInUrl: false,
          lock: processLock,
        },
      })
    : null;
