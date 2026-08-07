import { getIdentifier } from "@tauri-apps/api/app";
import type { ReactNode } from "react";

import { commands as iconCommands } from "@anlg/plugin-icon";

import { applyDocumentTheme, writeStoredThemePreference } from "./apply";
import {
  type AppIconPreference,
  normalizeAppIconPreference,
  resolveDockIconName,
} from "./icon";
import type { ThemePreference } from "./resolve";
import { useSettingsThemeReady } from "./use-settings-theme-ready";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function AppThemeProvider({ children }: { children: ReactNode }) {
  const theme = useConfigValue("theme") as ThemePreference;
  const appIcon = normalizeAppIconPreference(useConfigValue("app_icon"));
  const settingsReady = useSettingsThemeReady();

  return (
    <>
      {settingsReady ? (
        <ThemeSync
          key={`${theme}:${appIcon}`}
          theme={theme}
          appIcon={appIcon}
        />
      ) : null}
      {children}
    </>
  );
}

function ThemeSync({
  theme,
  appIcon,
}: {
  theme: ThemePreference;
  appIcon: AppIconPreference;
}) {
  useMountEffect(() => {
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
    let cancelled = false;

    const applySystemTheme = (systemIsDark: boolean) => {
      if (cancelled) {
        return;
      }

      void applyAppearance(theme, appIcon, systemIsDark);
    };

    const handleSystemThemeChange = ({ matches }: MediaQueryListEvent) => {
      applySystemTheme(matches);
    };

    systemTheme.addEventListener("change", handleSystemThemeChange);
    applySystemTheme(systemTheme.matches);

    return () => {
      cancelled = true;
      systemTheme.removeEventListener("change", handleSystemThemeChange);
    };
  });

  return null;
}

export async function applyThemePreference(
  theme: ThemePreference,
  appIcon: AppIconPreference = "default",
) {
  await applyAppearance(theme, appIcon, prefersDarkColorScheme());
}

async function applyAppearance(
  theme: ThemePreference,
  appIcon: AppIconPreference,
  systemIsDark: boolean,
) {
  applyDocumentTheme(theme, systemIsDark);
  writeStoredThemePreference(theme);
  await applyDockIcon(appIcon, theme, systemIsDark);
}

export async function applyAppIconPreference(
  appIcon: AppIconPreference,
  theme: ThemePreference = "system",
) {
  await applyDockIcon(appIcon, theme, prefersDarkColorScheme());
}

function prefersDarkColorScheme(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

async function applyDockIcon(
  appIcon: AppIconPreference,
  theme: ThemePreference,
  systemIsDark: boolean,
) {
  const appIdentifier = await getIdentifier().catch(
    () => "com.hyprnote.stable",
  );

  try {
    const result = await iconCommands.setDockIcon(
      resolveDockIconName(appIcon, theme, systemIsDark, appIdentifier),
    );
    if (result.status === "error") {
      console.error("[theme] failed to update Dock icon", result.error);
    }
  } catch (error) {
    console.error("[theme] failed to update Dock icon", error);
  }
}
