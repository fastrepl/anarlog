import { type ReactNode, useLayoutEffect } from "react";

import { applyDocumentTheme, writeStoredThemePreference } from "./apply";
import type { ThemePreference } from "./resolve";

import { useConfigValue } from "~/shared/config";

export function AppThemeProvider({ children }: { children: ReactNode }) {
  const theme = useConfigValue("theme") as ThemePreference;

  useLayoutEffect(() => {
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
  }, [theme]);

  return children;
}
