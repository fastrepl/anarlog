import { describe, expect, it } from "vitest";

import {
  normalizeThemePreference,
  resolveBootIsDark,
} from "./apply";

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
});
