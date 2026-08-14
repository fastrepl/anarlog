import { resolveIsDarkMode, type ThemePreference } from "./resolve";

export type AppIconPreference =
  | "default"
  | "stable"
  | "anagram"
  | "dev"
  | "staging"
  | "journal"
  | "notepad"
  | "stone"
  | "typewriter-key"
  | "walnut";

export function normalizeAppIconPreference(
  value: string | null | undefined,
): AppIconPreference {
  switch (value) {
    case "stable":
    case "anagram":
    case "dev":
    case "staging":
    case "journal":
    case "notepad":
    case "stone":
    case "typewriter-key":
    case "walnut":
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
  return hasDarkAppIconVariant(name) && resolveIsDarkMode(theme, systemIsDark)
    ? `${name}-dark`
    : name;
}

export function hasDarkAppIconVariant(
  name: Exclude<AppIconPreference, "default">,
): boolean {
  return (
    name === "stable" ||
    name === "anagram" ||
    name === "dev" ||
    name === "staging"
  );
}
