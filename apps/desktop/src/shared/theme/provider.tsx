import { type ReactNode, useLayoutEffect } from "react";

import { applyDocumentTheme, writeStoredThemePreference } from "./apply";
import type { ThemePreference } from "./resolve";
import { useSettingsThemeReady } from "./use-settings-theme-ready";

import { useConfigValue } from "~/shared/config";

export function AppThemeProvider({ children }: { children: ReactNode }) {
  const theme = useConfigValue("theme") as ThemePreference;
  const settingsReady = useSettingsThemeReady();

  useLayoutEffect(() => {
    if (!settingsReady) {
      return;
    }

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    const applyTheme = () => {
      applyDocumentTheme(theme);
      writeStoredThemePreference(theme);
    };

    applyTheme();

    if (theme !== "system") {
      return;
    }

    mediaQuery.addEventListener("change", applyTheme);
    return () => mediaQuery.removeEventListener("change", applyTheme);
  }, [theme, settingsReady]);

  return children;
}
