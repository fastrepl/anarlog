import { describe, expect, it } from "vitest";

import {
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

  it("follows the system appearance for the system theme", () => {
    expect(
      resolveDockIconName("anagram", "system", false, "com.hyprnote.dev"),
    ).toBe("anagram");
    expect(
      resolveDockIconName("anagram", "system", true, "com.hyprnote.dev"),
    ).toBe("anagram-dark");
    expect(
      resolveDockIconName("staging", "system", true, "com.hyprnote.stable"),
    ).toBe("staging-dark");
  });

  it("overrides the system appearance with an explicit theme", () => {
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
});
