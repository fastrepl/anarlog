import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const loadSettings = vi.hoisted(() => vi.fn());

vi.mock("@hypr/plugin-settings", () => ({
  commands: {
    load: loadSettings,
  },
}));

import {
  bootstrapThemeFromSettings,
  normalizeThemePreference,
  resolveBootIsDark,
  themePreferenceFromSettings,
} from "./apply";

function mockSystemTheme(prefersDark: boolean) {
  window.matchMedia = vi.fn().mockReturnValue({
    matches: prefersDark,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
}

beforeEach(() => {
  loadSettings.mockReset();
  localStorage.clear();
  document.documentElement.className = "";
  mockSystemTheme(false);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("normalizeThemePreference", () => {
  it("returns stored theme values", () => {
    expect(normalizeThemePreference("light")).toBe("light");
    expect(normalizeThemePreference("dark")).toBe("dark");
    expect(normalizeThemePreference("system")).toBe("system");
  });

  it("falls back to system for missing or invalid values", () => {
    expect(normalizeThemePreference(null)).toBe("system");
    expect(normalizeThemePreference("invalid")).toBe("system");
  });
});

describe("themePreferenceFromSettings", () => {
  it("reads the persisted general.theme value", () => {
    expect(
      themePreferenceFromSettings({
        general: { theme: "dark" },
      }),
    ).toBe("dark");
  });

  it("falls back to system when theme is missing", () => {
    expect(themePreferenceFromSettings({ general: {} })).toBe("system");
    expect(themePreferenceFromSettings(undefined)).toBe("system");
  });
});

describe("resolveBootIsDark", () => {
  it("honors explicit light and dark preferences", () => {
    expect(resolveBootIsDark("light", true)).toBe(false);
    expect(resolveBootIsDark("dark", false)).toBe(true);
  });

  it("follows system preference when stored theme is system or missing", () => {
    expect(resolveBootIsDark("system", true)).toBe(true);
    expect(resolveBootIsDark("system", false)).toBe(false);
    expect(resolveBootIsDark(null, true)).toBe(true);
    expect(resolveBootIsDark(null, false)).toBe(false);
  });

  it("treats invalid boot values like system to avoid theme flashes", () => {
    expect(resolveBootIsDark("legacy-value", true)).toBe(true);
    expect(resolveBootIsDark("legacy-value", false)).toBe(false);
  });
});

describe("bootstrapThemeFromSettings", () => {
  it("applies persisted settings before resolving when load is prompt", async () => {
    loadSettings.mockResolvedValue({
      status: "ok",
      data: { general: { theme: "dark" } },
    });

    await bootstrapThemeFromSettings({ timeoutMs: 100 });

    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(localStorage.getItem("hypr-theme")).toBe("dark");
  });

  it("does not hold startup past the deadline when settings load stalls", async () => {
    vi.useFakeTimers();

    let resolveLoad!: (value: {
      status: "ok";
      data: { general: { theme: "dark" } };
    }) => void;
    loadSettings.mockReturnValue(
      new Promise((resolve) => {
        resolveLoad = resolve;
      }),
    );

    const bootstrap = bootstrapThemeFromSettings({ timeoutMs: 20 });
    let resolved = false;
    void bootstrap.then(() => {
      resolved = true;
    });

    await vi.advanceTimersByTimeAsync(20);

    expect(resolved).toBe(true);
    expect(localStorage.getItem("hypr-theme")).toBe(null);

    resolveLoad({
      status: "ok",
      data: { general: { theme: "dark" } },
    });
    await Promise.resolve();

    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(localStorage.getItem("hypr-theme")).toBe("dark");
  });
});
