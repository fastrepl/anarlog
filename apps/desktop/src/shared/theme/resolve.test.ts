import { describe, expect, it } from "vitest";

import { resolveIsDarkMode } from "./resolve";

describe("resolveIsDarkMode", () => {
  it.each([
    ["dark", false, true],
    ["light", true, false],
    ["system", true, true],
    ["system", false, false],
  ] as const)(
    "resolves %s theme with system appearance %s to %s",
    (theme, prefersDark, expected) => {
      expect(resolveIsDarkMode(theme, prefersDark)).toBe(expected);
    },
  );
});
