import type { ConfigContext, ExpoConfig } from "expo/config";

type AppVariant = "dev" | "staging" | "stable";

const variants = {
  dev: {
    name: "Anarlog Dev",
    icon: "./assets/images/icon-dev.png",
    scheme: "anarlog-dev",
    bundleIdentifier: "so.anarlog.mobile.dev",
  },
  staging: {
    name: "Anarlog Staging",
    icon: "./assets/images/icon-staging.png",
    scheme: "anarlog-staging",
    bundleIdentifier: "so.anarlog.mobile.staging",
  },
  stable: {
    name: "Anarlog",
    icon: "./assets/images/icon.png",
    scheme: "anarlog",
    bundleIdentifier: "so.anarlog.mobile",
  },
} as const satisfies Record<
  AppVariant,
  {
    name: string;
    icon: string;
    scheme: string;
    bundleIdentifier: string;
  }
>;

export function resolveAppVariant(value = process.env.APP_VARIANT): AppVariant {
  if (value === undefined) return "dev";
  if (value === "dev" || value === "staging" || value === "stable") {
    return value;
  }
  throw new Error(`Invalid APP_VARIANT: ${value}`);
}

export default ({ config }: ConfigContext): ExpoConfig => {
  const appVariant = resolveAppVariant();
  const variant = variants[appVariant];
  const isDev = appVariant === "dev";

  return {
    ...config,
    name: variant.name,
    slug: config.slug ?? "anarlog-mobile",
    icon: variant.icon,
    scheme: variant.scheme,
    ios: {
      ...config.ios,
      icon: variant.icon,
      bundleIdentifier: variant.bundleIdentifier,
    },
    android: {
      ...config.android,
      icon: variant.icon,
      package: variant.bundleIdentifier,
      adaptiveIcon: undefined,
    },
    plugins: (config.plugins ?? []).map((plugin) => {
      const name = Array.isArray(plugin) ? plugin[0] : plugin;
      if (name === "expo-dev-client") {
        return [
          "expo-dev-client",
          { addGeneratedScheme: appVariant === "dev" },
        ];
      }
      if (name !== "expo-widgets") return plugin;

      return [
        "expo-widgets",
        {
          bundleIdentifier: `${variant.bundleIdentifier}.widgets`,
          groupIdentifier: `group.${variant.bundleIdentifier}`,
        },
      ];
    }),
    extra: {
      ...config.extra,
      appVariant,
      appScheme: variant.scheme,
      supabaseUrl:
        process.env.EXPO_PUBLIC_SUPABASE_URL ?? process.env.SUPABASE_URL ?? "",
      supabaseAnonKey:
        process.env.EXPO_PUBLIC_SUPABASE_ANON_KEY ??
        process.env.SUPABASE_ANON_KEY ??
        "",
      apiUrl:
        process.env.EXPO_PUBLIC_API_URL ??
        (isDev ? "http://localhost:3001" : "https://api.anarlog.so"),
      appUrl:
        process.env.EXPO_PUBLIC_APP_URL ??
        (isDev ? "http://localhost:3000" : "https://anarlog.so"),
      posthogApiKey:
        process.env.EXPO_PUBLIC_POSTHOG_API_KEY ??
        process.env.POSTHOG_API_KEY ??
        "",
      posthogHost:
        process.env.EXPO_PUBLIC_POSTHOG_HOST ?? "https://us.i.posthog.com",
      sentryDsn:
        process.env.EXPO_PUBLIC_SENTRY_DSN ?? process.env.SENTRY_DSN ?? "",
    },
  };
};
