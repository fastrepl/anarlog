export type AppIconPreference = "default" | "anagram";

export function normalizeAppIconPreference(
  value: string | null | undefined,
): AppIconPreference {
  return value === "anagram" ? "anagram" : "default";
}

export function resolveDockIconName(
  icon: AppIconPreference,
  isDark: boolean,
): string {
  const name = icon === "anagram" ? "anagram" : "stable";
  return isDark ? `${name}-dark` : name;
}
