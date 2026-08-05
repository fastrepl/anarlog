export type AppIconPreference =
  | "default"
  | "stable"
  | "anagram"
  | "dev"
  | "staging";

export type AppIconAppearance = "auto" | "light" | "dark";

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

export function normalizeAppIconAppearance(
  value: string | null | undefined,
): AppIconAppearance {
  return value === "light" || value === "dark" ? value : "auto";
}

/** `systemIsDark` is the Dock's appearance, not the in-app theme. */
export function resolveDockIconName(
  icon: AppIconPreference,
  appearance: AppIconAppearance,
  systemIsDark: boolean,
  appIdentifier: string,
): string {
  const name = resolveAppIconName(icon, appIdentifier);
  const isDark = appearance === "auto" ? systemIsDark : appearance === "dark";
  return isDark ? `${name}-dark` : name;
}
