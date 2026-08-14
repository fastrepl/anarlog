import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const themeState = vi.hoisted(() => ({
  settingsReady: false,
  theme: "system" as "light" | "dark" | "system",
  appIcon: "default" as "default" | "stable" | "anagram" | "dev" | "staging",
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
const setNativeTheme = vi.hoisted(() => vi.fn(async () => undefined));
const onThemeChanged = vi.hoisted(() =>
  vi.fn(async (_listener: (event: { payload: "light" | "dark" }) => void) =>
    vi.fn(),
  ),
);
const matchMedia = vi.hoisted(() => vi.fn());
const systemTheme = vi.hoisted(() => ({
  matches: false,
  addEventListener: vi.fn(),
  removeEventListener: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({ getIdentifier }));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    theme: nativeTheme,
    setTheme: setNativeTheme,
    onThemeChanged,
  }),
}));

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
    applyDocumentTheme.mockClear();
    writeStoredThemePreference.mockClear();
    setDockIcon.mockClear();
    getIdentifier.mockReset();
    getIdentifier.mockResolvedValue("com.hyprnote.stable");
    nativeTheme.mockReset();
    nativeTheme.mockResolvedValue("light");
    setNativeTheme.mockReset();
    setNativeTheme.mockResolvedValue(undefined);
    onThemeChanged.mockReset();
    onThemeChanged.mockResolvedValue(vi.fn());
    systemTheme.matches = false;
    systemTheme.addEventListener.mockReset();
    systemTheme.removeEventListener.mockReset();
    matchMedia.mockReset();
    matchMedia.mockReturnValue(systemTheme);
    document.documentElement.classList.remove("dark");
    window.matchMedia = matchMedia;
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
    expect(setNativeTheme).not.toHaveBeenCalled();
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
    expect(setNativeTheme).toHaveBeenCalledWith("light");
  });

  it("uses the native appearance for the system theme", async () => {
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
    expect(setNativeTheme).toHaveBeenCalledWith(null);
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

  it("uses the stable Dock icon when the app identifier is unavailable", async () => {
    getIdentifier.mockRejectedValue(new Error("unavailable"));
    themeState.settingsReady = true;

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));
  });

  it("pins the Dock icon to the dark theme against a light system", async () => {
    themeState.settingsReady = true;
    themeState.theme = "dark";
    themeState.appIcon = "anagram";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() =>
      expect(setDockIcon).toHaveBeenCalledWith("anagram-dark"),
    );
  });

  it("updates the Dock icon with the system theme when the appearance changes", async () => {
    themeState.settingsReady = true;
    themeState.theme = "system";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));

    const handleThemeChanged = onThemeChanged.mock.calls[0]?.[0];
    handleThemeChanged({ payload: "dark" });

    await waitFor(() =>
      expect(setDockIcon).toHaveBeenLastCalledWith("stable-dark"),
    );
    expect(applyDocumentTheme).toHaveBeenLastCalledWith("system", true);
  });

  it("rechecks native appearance when the webview reports a change", async () => {
    themeState.settingsReady = true;
    themeState.theme = "system";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));
    const handleWebviewThemeChanged =
      systemTheme.addEventListener.mock.calls[0]?.[1];
    applyDocumentTheme.mockClear();
    setDockIcon.mockClear();
    nativeTheme.mockClear();
    nativeTheme.mockResolvedValue("dark");

    handleWebviewThemeChanged({ matches: false });

    await waitFor(() =>
      expect(applyDocumentTheme).toHaveBeenCalledWith("system", true),
    );
    expect(nativeTheme).toHaveBeenCalledOnce();
    expect(setDockIcon).toHaveBeenCalledWith("stable-dark");
  });

  it("pins the native appearance for an explicit theme", async () => {
    themeState.settingsReady = true;
    themeState.theme = "light";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(setDockIcon).toHaveBeenCalledWith("stable"));
    expect(setNativeTheme).toHaveBeenCalledWith("light");
    expect(onThemeChanged).not.toHaveBeenCalled();
    expect(systemTheme.addEventListener).not.toHaveBeenCalled();
  });

  it("ignores stale system events after leaving system appearance", async () => {
    themeState.settingsReady = true;
    themeState.theme = "system";

    const { rerender } = render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(onThemeChanged).toHaveBeenCalledOnce());
    await waitFor(() => expect(applyDocumentTheme).toHaveBeenCalledOnce());
    const handleThemeChanged = onThemeChanged.mock.calls[0]?.[0];

    themeState.theme = "light";
    rerender(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );
    handleThemeChanged({ payload: "dark" });

    expect(applyDocumentTheme).toHaveBeenCalledTimes(2);
    expect(applyDocumentTheme).toHaveBeenLastCalledWith("light", false);
    expect(setDockIcon).toHaveBeenCalledWith("stable");
  });

  it("ignores a system event emitted while applying an explicit theme", async () => {
    themeState.settingsReady = true;
    themeState.theme = "system";

    render(
      <AppThemeProvider>
        <div>child</div>
      </AppThemeProvider>,
    );

    await waitFor(() => expect(onThemeChanged).toHaveBeenCalledOnce());
    const handleThemeChanged = onThemeChanged.mock.calls[0]?.[0];
    setNativeTheme.mockImplementationOnce(async () => {
      handleThemeChanged({ payload: "light" });
    });
    applyDocumentTheme.mockClear();
    writeStoredThemePreference.mockClear();

    await applyThemePreference("light");

    expect(applyDocumentTheme).toHaveBeenCalledOnce();
    expect(applyDocumentTheme).toHaveBeenCalledWith("light", false);
    expect(writeStoredThemePreference).toHaveBeenCalledOnce();
    expect(writeStoredThemePreference).toHaveBeenCalledWith("light");
  });

  it("applies a selected system theme and Dock icon from the native appearance", async () => {
    nativeTheme.mockResolvedValue("dark");

    await applyThemePreference("system");

    expect(setNativeTheme).toHaveBeenCalledWith(null);
    expect(nativeTheme).toHaveBeenCalledOnce();
    expect(applyDocumentTheme).toHaveBeenCalledWith("system", true);
    expect(writeStoredThemePreference).toHaveBeenCalledWith("system");
    expect(setDockIcon).toHaveBeenCalledWith("stable-dark");
  });

  it("applies an explicit selection and matching Dock icon immediately", async () => {
    await applyThemePreference("light");

    expect(setNativeTheme).toHaveBeenCalledWith("light");
    expect(nativeTheme).not.toHaveBeenCalled();
    expect(applyDocumentTheme).toHaveBeenCalledWith("light", false);
    expect(writeStoredThemePreference).toHaveBeenCalledWith("light");
    expect(setDockIcon).toHaveBeenCalledWith("stable");
  });

  it("pins the Dock icon when selecting the dark theme on a light system", async () => {
    await applyThemePreference("dark");

    expect(applyDocumentTheme).toHaveBeenCalledWith("dark", true);
    expect(setDockIcon).toHaveBeenCalledWith("stable-dark");
  });

  it("applies an icon selection using the system appearance", async () => {
    nativeTheme.mockResolvedValue("dark");

    await applyAppIconPreference("anagram");

    expect(nativeTheme).toHaveBeenCalledOnce();
    expect(setDockIcon).toHaveBeenCalledWith("anagram-dark");
  });

  it("applies an icon selection using an explicit theme", async () => {
    systemTheme.matches = true;

    await applyAppIconPreference("anagram", "light");

    expect(setDockIcon).toHaveBeenCalledWith("anagram");
  });
});
