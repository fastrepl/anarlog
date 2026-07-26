import AsyncStorage from "@react-native-async-storage/async-storage";
import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import { AppState } from "react-native";

import { env, hasSupabaseEnv } from "@/lib/env";

const firstHostnameLabel = (url: string): string =>
  url
    .replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, "")
    .split(/[/:?#]/)[0]!
    .split(".")[0]!;

export const authStorageKey = `sb-${firstHostnameLabel(
  env.supabaseUrl || "local",
)}-auth-token`;

export const supabase: SupabaseClient | null = hasSupabaseEnv
  ? createClient(env.supabaseUrl, env.supabaseAnonKey, {
      auth: {
        storage: AsyncStorage,
        storageKey: authStorageKey,
        autoRefreshToken: true,
        persistSession: true,
        detectSessionInUrl: false,
      },
    })
  : null;

if (supabase) {
  const client = supabase;
  AppState.addEventListener("change", (state) => {
    if (state === "active") {
      void client.auth.startAutoRefresh();
    } else {
      void client.auth.stopAutoRefresh();
    }
  });
}
