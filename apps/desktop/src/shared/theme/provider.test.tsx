import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const themeState = vi.hoisted(() => ({
  settingsReady: false,
  theme: "system" as "light" | "dark" | "system",
  appIcon: "default" as "default" | "stable" | "anagram" | "dev" | "staging",
  appIconAppearance: "auto" as "auto" | "light" | "dark",
}));

const applyDocumentTheme = vi.hoisted(() =>
  vi.fn((theme: string, prefersDark?: boolean) =>
    theme === "dark" ? true : theme === "system" && prefersDark === true,
  ),
);
const writeStoredThemePreference = vi.hoisted(() => vi.fn());
const setDockIcon = vi.hoisted(() =>
  vi.fn(async () => ({ status: "ok", data: null })),
);
const getIdentifier = vi.hoisted(() =>
  vi.fn(async () => "com.hyprnote.stable"),
);
const nativeTheme = vi.hoisted(() => vi.fn(async () => "light"));
const onThemeChanged = vi.hoisted(() =>
  vi.fn(async (_listener: (event: { payload: "light" | "dark" }) => void) =>
    vi.fn(),
  ),
);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    theme: nativeTheme,
    onThemeChanged,
  }),
}));

vi.mock("@tauri-apps/api/app", () => ({ getIdentifier }));

vi.mock("@anlg/plugin-icon", () => ({
  commands: { setDockIcon },
}));

vi.mock("./apply", () => ({
  applyDocumentTheme,
  writeStoredThemePreference,
}));

vi.mock("./use-settings-theme-ready", () => ({
  useSettingsThemeReady: () => themeState.settingsReady,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => {
    if (key === "app_icon") {
      return themeState.appIcon;
    }
    if (key === "app_icon_appearance") {
      return themeState.appIconAppearance;
    }
    return themeState.theme;
  },
}));

import {
  AppThemeProvider,
  applyAppIconPreference,
  applyThemePreference,
} from "./provider";

describe("AppThemeProvider", () => {
  beforeEach(() => {
    cleanup();
    themeState.settingsReady = false;
    themeState.theme = "system";
    themeState.appIcon = "default";
    themeState.appIconAppearance = "auto";
    applyDocumentTheme.mockClear();
    writeStoredThemePreference.mockClear();
    setDockIcon.mockClear();
    getIdentifier.mockReset();
    getIdentifier.mockResolvedValue("com.hyprnote.stable");
    nativeTheme.mockReset();
    nativeTheme.mockResolvedValue("light");
    onThemeChanged.mockReset();
    onThemeChanged.mockResolvedValue(vi.fn());
    document.documentElement.classList.remove("dark");
    window.matchMedia = vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("does not clobber the boot theme before settings hydrate", () => {
    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    expect(applyDocumentTheme).not.toHaveBeenCalled();
    expect(writeStoredThemePreference).not.toHaveBeenCalled();
    expect(setDockIcon).not.toHaveBeenCalled();
  });

  it("applies the hydrated settings theme once SQLite is ready", async () => {
    themeState.settingsReady = true;
    themeState.theme = "light";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() =>
      expect(applyDocumentTheme).toHaveBeenCalledWith("light", false),
    );
    expect(writeStoredThemePreference).toHaveBeenCalledWith("light");
    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));
  });

  it("uses the native window appearance for the system theme", async () => {
    themeState.settingsReady = true;
    themeState.theme = "system";
    nativeTheme.mockResolvedValue("dark");

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() =>
      expect(applyDocumentTheme).toHaveBeenCalledWith("system", true),
    );
    expect(writeStoredThemePreference).toHaveBeenCalledWith("system");
    await waitFor(() =>
      expect(setDockIcon).toHaveBeenCalledWith("stable-dark"),
    );
  });

  it("uses the channel-specific default Dock icon", async () => {
    getIdentifier.mockResolvedValue("com.hyprnote.staging");
    themeState.settingsReady = true;

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("staging"));
  });

  it("applies the selected icon variant for the system appearance", async () => {
    themeState.settingsReady = true;
    themeState.theme = "dark";
    themeState.appIcon = "anagram";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("anagram"));
  });

  it("updates the Dock icon when the system appearance changes", async () => {
    let handleThemeChanged!: (event: { payload: "light" | "dark" }) => void;
    onThemeChanged.mockImplementationOnce(async (listener) => {
      handleThemeChanged = listener;
      return vi.fn();
    });
    themeState.settingsReady = true;
    themeState.theme = "light";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));

    handleThemeChanged({ payload: "dark" });

    await waitFor(() =>
      expect(setDockIcon).toHaveBeenLastCalledWith("stable-dark"),
    );
    expect(applyDocumentTheme).toHaveBeenLastCalledWith("light", true);
  });

  it("ignores a stale native theme after leaving system appearance", async () => {
    let resolveNativeTheme!: (theme: string) => void;
    nativeTheme.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveNativeTheme = resolve;
        }),
    );
    themeState.settingsReady = true;
    themeState.theme = "system";

    const { rerender } = render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(nativeTheme).toHaveBeenCalledOnce());

    themeState.theme = "light";
    rerender(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );
    resolveNativeTheme("dark");
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(applyDocumentTheme).toHaveBeenCalledTimes(1);
    expect(applyDocumentTheme).toHaveBeenCalledWith("light", false);
    expect(setDockIcon).toHaveBeenCalledWith("stable");
  });

  it("applies a selected system theme and Dock icon from the native source", async () => {
    nativeTheme.mockResolvedValue("dark");

    await applyThemePreference("system");

    expect(nativeTheme).toHaveBeenCalledOnce();
    expect(applyDocumentTheme).toHaveBeenCalledWith("system", true);
    expect(writeStoredThemePreference).toHaveBeenCalledWith("system");
    expect(setDockIcon).toHaveBeenCalledWith("stable-dark");
  });

  it("applies an explicit selection and matching Dock icon immediately", async () => {
    await applyThemePreference("light");

    expect(nativeTheme).toHaveBeenCalledOnce();
    expect(applyDocumentTheme).toHaveBeenCalledWith("light", false);
    expect(writeStoredThemePreference).toHaveBeenCalledWith("light");
    expect(setDockIcon).toHaveBeenCalledWith("stable");
  });

  it("applies an icon selection using the system appearance", async () => {
    nativeTheme.mockResolvedValue("dark");

    await applyAppIconPreference("anagram");

    expect(setDockIcon).toHaveBeenCalledWith("anagram-dark");
  });

  it("pins the Dock icon to an explicit appearance against the system", async () => {
    themeState.settingsReady = true;
    themeState.theme = "light";
    themeState.appIconAppearance = "dark";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() =>
      expect(setDockIcon).toHaveBeenCalledWith("stable-dark"),
    );
  });

  it("keeps an explicit appearance when the system appearance changes", async () => {
    let handleThemeChanged!: (event: { payload: "light" | "dark" }) => void;
    onThemeChanged.mockImplementationOnce(async (listener) => {
      handleThemeChanged = listener;
      return vi.fn();
    });
    themeState.settingsReady = true;
    themeState.appIconAppearance = "light";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));

    handleThemeChanged({ payload: "dark" });

    await waitFor(() =>
      expect(applyDocumentTheme).toHaveBeenLastCalledWith("system", true),
    );
    expect(setDockIcon).toHaveBeenLastCalledWith("stable");
  });

  it("applies an explicit appearance for a direct icon selection", async () => {
    nativeTheme.mockResolvedValue("dark");

    await applyAppIconPreference("anagram", "light");

    expect(setDockIcon).toHaveBeenCalledWith("anagram");
  });
});
