import Constants from "expo-constants";

const extra = (Constants.expoConfig?.extra ?? {}) as Record<string, unknown>;

const read = (key: string, fallback = ""): string => {
  const value = extra[key];
  return typeof value === "string" && value ? value : fallback;
};

export const env = {
  appVariant: read("appVariant", __DEV__ ? "dev" : "stable"),
  appScheme: read("appScheme", "anarlog"),
  supabaseUrl: read("supabaseUrl"),
  supabaseAnonKey: read("supabaseAnonKey"),
  apiUrl: read("apiUrl", "https://api.anarlog.so"),
  r2BackupWorkerUrl: read(
    "r2BackupWorkerUrl",
    "https://anarlog-r2-backup.doantrang3129393.workers.dev",
  ),
  appUrl: read("appUrl", "https://anarlog.so"),
  posthogApiKey: read("posthogApiKey"),
  posthogHost: read("posthogHost", "https://us.i.posthog.com"),
  sentryDsn: read("sentryDsn"),
  googleIosClientId: read("googleIosClientId"),
  googleIosScheme: read("googleIosScheme"),
};

export const hasSupabaseEnv = Boolean(env.supabaseUrl && env.supabaseAnonKey);
