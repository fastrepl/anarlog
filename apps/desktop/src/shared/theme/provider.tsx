import { getIdentifier } from "@tauri-apps/api/app";
import {
  getCurrentWindow,
  type Theme,
  type Window,
} from "@tauri-apps/api/window";
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

let activeThemePreference: ThemePreference = "system";

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
    activeThemePreference = theme;
    const appWindow = getCurrentWindow();
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const applySystemTheme = (systemIsDark: boolean) => {
      if (cancelled || activeThemePreference !== theme) {
        return;
      }

      void applyAppearance(theme, appIcon, systemIsDark);
    };

    const refreshSystemTheme = async () => {
      applySystemTheme(await readSystemIsDark(appWindow));
    };

    if (theme !== "system") {
      applySystemTheme(theme === "dark");
      void setNativeThemePreference(appWindow, theme);

      return () => {
        cancelled = true;
      };
    }

    const handleSystemThemeChange = () => {
      void refreshSystemTheme();
    };

    systemTheme.addEventListener("change", handleSystemThemeChange);
    window.addEventListener("focus", handleSystemThemeChange);

    void (async () => {
      await setNativeThemePreference(appWindow, theme);
      unlisten = await appWindow.onThemeChanged(({ payload }) => {
        applySystemTheme(payload === "dark");
      });

      if (cancelled) {
        unlisten();
        return;
      }

      await refreshSystemTheme();
    })().catch((error) => {
      if (!cancelled) {
        console.error("[theme] failed to follow system appearance", error);
        applySystemTheme(systemTheme.matches);
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
      systemTheme.removeEventListener("change", handleSystemThemeChange);
      window.removeEventListener("focus", handleSystemThemeChange);
    };
  });

  return null;
}

export async function applyThemePreference(
  theme: ThemePreference,
  appIcon: AppIconPreference = "default",
) {
  activeThemePreference = theme;
  const appWindow = getCurrentWindow();

  if (theme !== "system") {
    await Promise.all([
      setNativeThemePreference(appWindow, theme),
      applyAppearance(theme, appIcon, theme === "dark"),
    ]);
    return;
  }

  await setNativeThemePreference(appWindow, theme);
  await applyAppearance(theme, appIcon, await readSystemIsDark(appWindow));
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
  await applyDockIcon(
    appIcon,
    theme,
    theme === "system" ? await readSystemIsDark() : theme === "dark",
  );
}

async function setNativeThemePreference(
  appWindow: Window,
  theme: ThemePreference,
) {
  try {
    await appWindow.setTheme(theme === "system" ? null : theme);
  } catch (error) {
    console.error("[theme] failed to update native appearance", error);
  }
}

async function readSystemIsDark(
  appWindow: Window = getCurrentWindow(),
): Promise<boolean> {
  try {
    return isDarkTheme(await appWindow.theme());
  } catch (error) {
    console.error("[theme] failed to read system appearance", error);
    return prefersDarkColorScheme();
  }
}

function isDarkTheme(theme: Theme | null): boolean {
  return theme === null ? prefersDarkColorScheme() : theme === "dark";
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
