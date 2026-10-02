import { describe, expect, test } from "vitest";

import { resolveDisplayLocale } from "./locales";

describe("resolveDisplayLocale", () => {
  test("resolves the canonical Filipino locale to the Tagalog catalog", () => {
    expect(resolveDisplayLocale("fil-PH")).toBe("tl");
  });

  test("uses base language for regional variants", () => {
    expect(resolveDisplayLocale("pt-BR")).toBe("pt");
    expect(resolveDisplayLocale("zh-Hans")).toBe("zh");
  });

  test("falls back to English for unsupported or invalid values", () => {
    expect(resolveDisplayLocale("eo")).toBe("en");
    expect(resolveDisplayLocale("not a locale")).toBe("en");
  });
});
