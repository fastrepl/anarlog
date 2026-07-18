import { getCurrentWindow, type Theme } from "@tauri-apps/api/window";
import type { ReactNode } from "react";

import { commands as iconCommands } from "@hypr/plugin-icon";

import { applyDocumentTheme, writeStoredThemePreference } from "./apply";
import type { ThemePreference } from "./resolve";
import { useSettingsThemeReady } from "./use-settings-theme-ready";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function AppThemeProvider({ children }: { children: ReactNode }) {
  const theme = useConfigValue("theme") as ThemePreference;
  const settingsReady = useSettingsThemeReady();

  return (
    <>
      {settingsReady ? <ThemeSync key={theme} theme={theme} /> : null}
      {children}
    </>
  );
}

function ThemeSync({ theme }: { theme: ThemePreference }) {
  useMountEffect(() => {
    if (theme !== "system") {
      applyAppTheme(theme);
      return;
    }

    const appWindow = getCurrentWindow();
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const applySystemTheme = (systemTheme: Theme | null) => {
      if (cancelled) {
        return;
      }

      applyAppTheme(theme, systemTheme === "dark");
    };

    void (async () => {
      unlisten = await appWindow.onThemeChanged(({ payload }) => {
        applySystemTheme(payload);
      });

      if (cancelled) {
        unlisten();
        return;
      }

      applySystemTheme(await appWindow.theme());
    })().catch((error) => {
      if (!cancelled) {
        console.error("[theme] failed to read system appearance", error);
        applyAppTheme(theme);
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  });

  return null;
}

export async function applyThemePreference(theme: ThemePreference) {
  if (theme !== "system") {
    applyAppTheme(theme);
    return;
  }

  try {
    const systemTheme = await getCurrentWindow().theme();
    applyAppTheme(theme, systemTheme === "dark");
  } catch (error) {
    console.error("[theme] failed to read system appearance", error);
    applyAppTheme(theme);
  }
}

function applyAppTheme(theme: ThemePreference, prefersDark?: boolean) {
  const isDark =
    prefersDark === undefined
      ? applyDocumentTheme(theme)
      : applyDocumentTheme(theme, prefersDark);
  writeStoredThemePreference(theme);

  void iconCommands
    .setDockIcon(isDark ? "stable-dark" : "stable")
    .then((result) => {
      if (result.status === "error") {
        console.error("[theme] failed to update Dock icon", result.error);
      }
    })
    .catch((error) => {
      console.error("[theme] failed to update Dock icon", error);
    });
}
