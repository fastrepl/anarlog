import type { ConfigContext, ExpoConfig } from "expo/config";

const defaultApiUrl =
  process.env.NODE_ENV === "development"
    ? "http://localhost:3001"
    : "https://api.anarlog.so";

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
    apiUrl: process.env.EXPO_PUBLIC_API_URL ?? defaultApiUrl,
    appUrl: process.env.EXPO_PUBLIC_APP_URL ?? "http://localhost:3000",
    posthogApiKey:
      process.env.EXPO_PUBLIC_POSTHOG_API_KEY ??
      process.env.POSTHOG_API_KEY ??
      "",
    posthogHost:
      process.env.EXPO_PUBLIC_POSTHOG_HOST ?? "https://us.i.posthog.com",
    sentryDsn:
      process.env.EXPO_PUBLIC_SENTRY_DSN ?? process.env.SENTRY_DSN ?? "",
  },
});
