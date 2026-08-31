import Constants from "expo-constants";

const extra = (Constants.expoConfig?.extra ?? {}) as Record<string, unknown>;

const read = (key: string, fallback = ""): string => {
  const value = extra[key];
  return typeof value === "string" && value ? value : fallback;
};

export const env = {
  supabaseUrl: read("supabaseUrl"),
  supabaseAnonKey: read("supabaseAnonKey"),
  apiUrl: read("apiUrl", "https://api.anarlog.so"),
  appUrl: read("appUrl", "https://anarlog.so"),
  posthogApiKey: read("posthogApiKey"),
  posthogHost: read("posthogHost", "https://us.i.posthog.com"),
  sentryDsn: read("sentryDsn"),
};

export const hasSupabaseEnv = Boolean(env.supabaseUrl && env.supabaseAnonKey);
