import { getIdentifier } from "@tauri-apps/api/app";
import type { ReactNode } from "react";

import { commands as iconCommands } from "@anlg/plugin-icon";

import { applyDocumentTheme, writeStoredThemePreference } from "./apply";
import {
  type AppIconAppearance,
  type AppIconPreference,
  normalizeAppIconAppearance,
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
  const appIconAppearance = normalizeAppIconAppearance(
    useConfigValue("app_icon_appearance"),
  );
  const settingsReady = useSettingsThemeReady();

  return (
    <>
      {settingsReady ? (
        <ThemeSync
          key={`${theme}:${appIcon}:${appIconAppearance}`}
          theme={theme}
          appIcon={appIcon}
          appIconAppearance={appIconAppearance}
        />
      ) : null}
      {children}
    </>
  );
}

function ThemeSync({
  theme,
  appIcon,
  appIconAppearance,
}: {
  theme: ThemePreference;
  appIcon: AppIconPreference;
  appIconAppearance: AppIconAppearance;
}) {
  useMountEffect(() => {
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
    let cancelled = false;

    const applySystemTheme = (systemIsDark: boolean) => {
      if (cancelled) {
        return;
      }

      void applyAppearance(theme, appIcon, appIconAppearance, systemIsDark);
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
  appIconAppearance: AppIconAppearance = "auto",
) {
  await applyAppearance(
    theme,
    appIcon,
    appIconAppearance,
    prefersDarkColorScheme(),
  );
}

async function applyAppearance(
  theme: ThemePreference,
  appIcon: AppIconPreference,
  appIconAppearance: AppIconAppearance,
  systemIsDark: boolean,
) {
  applyDocumentTheme(theme, systemIsDark);
  writeStoredThemePreference(theme);
  await applyDockIcon(appIcon, appIconAppearance, systemIsDark);
}

export async function applyAppIconPreference(
  appIcon: AppIconPreference,
  appIconAppearance: AppIconAppearance = "auto",
) {
  await applyDockIcon(appIcon, appIconAppearance, prefersDarkColorScheme());
}

function prefersDarkColorScheme(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

async function applyDockIcon(
  appIcon: AppIconPreference,
  appearance: AppIconAppearance,
  systemIsDark: boolean,
) {
  const appIdentifier = await getIdentifier().catch(() => "com.hyprnote.dev");

  try {
    const result = await iconCommands.setDockIcon(
      resolveDockIconName(appIcon, appearance, systemIsDark, appIdentifier),
    );
    if (result.status === "error") {
      console.error("[theme] failed to update Dock icon", result.error);
    }
  } catch (error) {
    console.error("[theme] failed to update Dock icon", error);
  }
}
