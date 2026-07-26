import Constants from "expo-constants";

const extra = (Constants.expoConfig?.extra ?? {}) as Record<string, unknown>;

const read = (key: string, fallback = ""): string => {
  const value = extra[key];
  return typeof value === "string" && value ? value : fallback;
};

export const env = {
  supabaseUrl: read("supabaseUrl"),
  supabaseAnonKey: read("supabaseAnonKey"),
  apiUrl: read("apiUrl", "http://localhost:3001"),
  appUrl: read("appUrl", "http://localhost:3000"),
};

export const hasSupabaseEnv = Boolean(env.supabaseUrl && env.supabaseAnonKey);
