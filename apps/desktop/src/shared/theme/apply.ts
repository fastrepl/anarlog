import { resolveIsDarkMode, type ThemePreference } from "./resolve";

const THEME_STORAGE_KEY = "hypr-theme";

/** Keep `public/theme-boot.js` aligned with normalizeThemePreference + resolveIsDarkMode. */

export function readStoredThemePreference(): ThemePreference {
  const stored = localStorage.getItem(THEME_STORAGE_KEY);
  return normalizeThemePreference(stored);
}

export function normalizeThemePreference(
  stored: string | null,
): ThemePreference {
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return "system";
}

export function resolveBootIsDark(
  stored: string | null,
  prefersDark: boolean,
): boolean {
  return resolveIsDarkMode(normalizeThemePreference(stored), prefersDark);
}

export function writeStoredThemePreference(theme: ThemePreference): void {
  localStorage.setItem(THEME_STORAGE_KEY, theme);
}

export function applyDocumentTheme(theme: ThemePreference): boolean {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const isDark = resolveIsDarkMode(theme, prefersDark);
  document.documentElement.classList.toggle("dark", isDark);
  return isDark;
}
