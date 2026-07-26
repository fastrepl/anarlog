import type { ConfigContext, ExpoConfig } from "expo/config";

export default ({ config }: ConfigContext): ExpoConfig => ({
  ...config,
  name: config.name ?? "Anarlog",
  slug: config.slug ?? "anarlog-mobile",
  extra: {
    ...config.extra,
    supabaseUrl:
      process.env.EXPO_PUBLIC_SUPABASE_URL ?? process.env.SUPABASE_URL ?? "",
    supabaseAnonKey:
      process.env.EXPO_PUBLIC_SUPABASE_ANON_KEY ??
      process.env.SUPABASE_ANON_KEY ??
      "",
    apiUrl: process.env.EXPO_PUBLIC_API_URL ?? "http://localhost:3001",
    appUrl: process.env.EXPO_PUBLIC_APP_URL ?? "http://localhost:3000",
  },
});
