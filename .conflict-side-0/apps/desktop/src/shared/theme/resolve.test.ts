import { describe, expect, it } from "vitest";

import { resolveIsDarkMode } from "./resolve";

describe("resolveIsDarkMode", () => {
  it("returns true for dark theme", () => {
    expect(resolveIsDarkMode("dark", false)).toBe(true);
  });

  it("returns false for light theme", () => {
    expect(resolveIsDarkMode("light", true)).toBe(false);
  });

  it("follows system preference for system theme", () => {
    expect(resolveIsDarkMode("system", true)).toBe(true);
    expect(resolveIsDarkMode("system", false)).toBe(false);
  });
});
