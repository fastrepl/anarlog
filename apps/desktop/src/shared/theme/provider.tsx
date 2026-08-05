import { getIdentifier } from "@tauri-apps/api/app";
import { getCurrentWindow, type Theme } from "@tauri-apps/api/window";
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
    const appWindow = getCurrentWindow();
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const applySystemTheme = (systemTheme: Theme | null) => {
      if (cancelled) {
        return;
      }

      void applyAppearance(
        theme,
        appIcon,
        appIconAppearance,
        isSystemDark(systemTheme),
      );
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
        applyAppearance(
          theme,
          appIcon,
          appIconAppearance,
          prefersDarkColorScheme(),
        );
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
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
    await readSystemIsDark(),
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
  await applyDockIcon(appIcon, appIconAppearance, await readSystemIsDark());
}

async function readSystemIsDark(): Promise<boolean> {
  try {
    return isSystemDark(await getCurrentWindow().theme());
  } catch (error) {
    console.error("[theme] failed to read system appearance", error);
    return prefersDarkColorScheme();
  }
}

function isSystemDark(theme: Theme | null): boolean {
  return theme === null ? prefersDarkColorScheme() : theme === "dark";
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
