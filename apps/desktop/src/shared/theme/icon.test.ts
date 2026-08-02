import { describe, expect, it } from "vitest";

import { normalizeAppIconPreference, resolveDockIconName } from "./icon";

describe("app icon preference", () => {
  it("falls back to the default icon for unknown values", () => {
    expect(normalizeAppIconPreference(undefined)).toBe("default");
    expect(normalizeAppIconPreference("unknown")).toBe("default");
  });

  it("resolves light and dark Dock resources", () => {
    expect(resolveDockIconName("default", false)).toBe("stable");
    expect(resolveDockIconName("default", true)).toBe("stable-dark");
    expect(resolveDockIconName("anagram", false)).toBe("anagram");
    expect(resolveDockIconName("anagram", true)).toBe("anagram-dark");
  });
});
