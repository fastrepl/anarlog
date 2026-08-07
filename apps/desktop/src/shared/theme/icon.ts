import { resolveIsDarkMode, type ThemePreference } from "./resolve";

export type AppIconPreference =
  | "default"
  | "stable"
  | "anagram"
  | "dev"
  | "staging";

export function normalizeAppIconPreference(
  value: string | null | undefined,
): AppIconPreference {
  switch (value) {
    case "stable":
    case "anagram":
    case "dev":
    case "staging":
      return value;
    default:
      return "default";
  }
}

export function resolveAppIconName(
  icon: AppIconPreference,
  appIdentifier: string,
): Exclude<AppIconPreference, "default"> {
  if (icon !== "default") {
    return icon;
  }
  if (appIdentifier.endsWith(".dev")) {
    return "dev";
  }
  if (appIdentifier.endsWith(".staging")) {
    return "staging";
  }
  return "stable";
}

/** `systemIsDark` is the Dock's appearance, used when the theme follows the system. */
export function resolveDockIconName(
  icon: AppIconPreference,
  theme: ThemePreference,
  systemIsDark: boolean,
  appIdentifier: string,
): string {
  const name = resolveAppIconName(icon, appIdentifier);
  return resolveIsDarkMode(theme, systemIsDark) ? `${name}-dark` : name;
}
