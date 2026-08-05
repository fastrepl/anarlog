import { describe, expect, it } from "vitest";

import {
  normalizeAppIconAppearance,
  normalizeAppIconPreference,
  resolveAppIconName,
  resolveDockIconName,
} from "./icon";

describe("app icon preference", () => {
  it("falls back to the default icon for unknown values", () => {
    expect(normalizeAppIconPreference(undefined)).toBe("default");
    expect(normalizeAppIconPreference("unknown")).toBe("default");
    expect(normalizeAppIconPreference("dev")).toBe("dev");
    expect(normalizeAppIconPreference("staging")).toBe("staging");
  });

  it("resolves the default icon from the app channel", () => {
    expect(resolveAppIconName("default", "com.hyprnote.stable")).toBe("stable");
    expect(resolveAppIconName("default", "com.hyprnote.staging")).toBe(
      "staging",
    );
    expect(resolveAppIconName("default", "com.hyprnote.dev")).toBe("dev");
  });

  it("follows the system appearance when the icon appearance is auto", () => {
    expect(
      resolveDockIconName("anagram", "auto", false, "com.hyprnote.dev"),
    ).toBe("anagram");
    expect(
      resolveDockIconName("anagram", "auto", true, "com.hyprnote.dev"),
    ).toBe("anagram-dark");
    expect(
      resolveDockIconName("staging", "auto", true, "com.hyprnote.stable"),
    ).toBe("staging-dark");
  });

  it("overrides the system appearance with an explicit icon appearance", () => {
    expect(
      resolveDockIconName("anagram", "dark", false, "com.hyprnote.dev"),
    ).toBe("anagram-dark");
    expect(
      resolveDockIconName("anagram", "light", true, "com.hyprnote.dev"),
    ).toBe("anagram");
    expect(
      resolveDockIconName("default", "dark", false, "com.hyprnote.stable"),
    ).toBe("stable-dark");
  });

  it("falls back to auto for unknown icon appearance values", () => {
    expect(normalizeAppIconAppearance(undefined)).toBe("auto");
    expect(normalizeAppIconAppearance("system")).toBe("auto");
    expect(normalizeAppIconAppearance("light")).toBe("light");
    expect(normalizeAppIconAppearance("dark")).toBe("dark");
  });
});
