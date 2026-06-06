import { resolveIsDarkMode, type ThemePreference } from "./resolve";

const THEME_STORAGE_KEY = "hypr-theme";

export function readStoredThemePreference(): ThemePreference | null {
  const stored = localStorage.getItem(THEME_STORAGE_KEY);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return null;
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
