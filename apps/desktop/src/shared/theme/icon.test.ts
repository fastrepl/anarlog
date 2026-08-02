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

  it("resolves explicit light and dark Dock resources", () => {
    expect(resolveDockIconName("anagram", false, "com.hyprnote.dev")).toBe(
      "anagram",
    );
    expect(resolveDockIconName("anagram", true, "com.hyprnote.dev")).toBe(
      "anagram-dark",
    );
    expect(resolveDockIconName("staging", true, "com.hyprnote.stable")).toBe(
      "staging-dark",
    );
  });
});
