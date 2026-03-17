import {
  createClient,
  processLock,
  type SupabaseClient,
  type SupportedStorage,
} from "@supabase/supabase-js";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

import { commands as authCommands } from "@hypr/plugin-auth";

import { env } from "~/env";

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
    const result = await authCommands.removeItem(key);
    if (result.status === "error") {
      throw new Error(`auth storage removeItem failed: ${result.error}`);
    }
  },
};

export const supabase: SupabaseClient | null =
  env.VITE_SUPABASE_URL && env.VITE_SUPABASE_ANON_KEY
    ? createClient(env.VITE_SUPABASE_URL, env.VITE_SUPABASE_ANON_KEY, {
        global: {
          fetch: tauriFetch,
        },
        auth: {
          storage: tauriStorage,
          autoRefreshToken: true,
          persistSession: true,
          detectSessionInUrl: false,
          lock: processLock,
        },
      })
    : null;
