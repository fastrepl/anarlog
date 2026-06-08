import { commands as settingsCommands } from "@hypr/plugin-settings";

import { resolveIsDarkMode, type ThemePreference } from "./resolve";

const THEME_STORAGE_KEY = "hypr-theme";
const THEME_BOOTSTRAP_TIMEOUT_MS = 150;

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
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Ignore unavailable storage in tests or restricted webviews.
  }
}

export function applyDocumentTheme(theme: ThemePreference): boolean {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const isDark = resolveIsDarkMode(theme, prefersDark);
  document.documentElement.classList.toggle("dark", isDark);
  return isDark;
}

export function themePreferenceFromSettings(
  settings: Record<string, unknown> | undefined,
): ThemePreference {
  const general = settings?.general;
  const theme =
    general && typeof general === "object" && "theme" in general
      ? (general as { theme?: unknown }).theme
      : null;

  return normalizeThemePreference(typeof theme === "string" ? theme : null);
}

async function loadThemeFromSettings(): Promise<void> {
  try {
    const result = await settingsCommands.load();
    if (result.status !== "ok") {
      return;
    }

    const preference = themePreferenceFromSettings(
      result.data as Record<string, unknown>,
    );
    applyDocumentTheme(preference);
    writeStoredThemePreference(preference);
  } catch {
    // Non-Tauri dev sessions can skip persisted settings bootstrap.
  }
}

export async function bootstrapThemeFromSettings({
  timeoutMs = THEME_BOOTSTRAP_TIMEOUT_MS,
}: {
  timeoutMs?: number;
} = {}): Promise<void> {
  const themeLoad = loadThemeFromSettings();

  if (timeoutMs <= 0) {
    await themeLoad;
    return;
  }

  await Promise.race([
    themeLoad,
    new Promise<void>((resolve) => setTimeout(resolve, timeoutMs)),
  ]);
}
