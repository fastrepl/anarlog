import { describe, expect, it } from "vitest";

import {
  appendPreferredNamesGuidance,
  formatPreferredNamesGuidance,
} from "./preferred-names";

describe("preferred names guidance", () => {
  it("formats dictionary terms as exact-spelling instructions", () => {
    expect(formatPreferredNamesGuidance(["Anarlog", "Char", "anarlog"])).toBe(
      `# Preferred Names

Use these names and terms exactly when they appear, even if the transcript or notes spell them differently:
- Anarlog
- Char`,
    );
  });

  it("omits empty dictionary lists", () => {
    expect(formatPreferredNamesGuidance([])).toBe("");
    expect(appendPreferredNamesGuidance("Base prompt", [])).toBe("Base prompt");
  });

  it("appends preferred names after the rendered prompt", () => {
    expect(appendPreferredNamesGuidance("Base prompt", ["Anarlog"])).toBe(
      `Base prompt

# Preferred Names

Use these names and terms exactly when they appear, even if the transcript or notes spell them differently:
- Anarlog`,
    );
  });
});
