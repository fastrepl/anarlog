import { describe, expect, test } from "vitest";

import { resolveDisplayLocale, SUPPORTED_DISPLAY_LOCALES } from "./locales";

describe("resolveDisplayLocale", () => {
  test("uses exact supported locales", () => {
    expect(resolveDisplayLocale("es")).toBe("es");
  });

  test("includes broad settings main-language options", () => {
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("ar");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("hi");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("nl");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("pl");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("ru");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("tr");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("uk");
    expect(SUPPORTED_DISPLAY_LOCALES).toContain("vi");
  });

  test("uses base language for regional variants", () => {
    expect(resolveDisplayLocale("pt-BR")).toBe("pt");
    expect(resolveDisplayLocale("zh-Hans")).toBe("zh");
  });

  test("falls back to English for unsupported languages", () => {
    expect(resolveDisplayLocale("eo")).toBe("en");
  });

  test("falls back to English for invalid values", () => {
    expect(resolveDisplayLocale("not a locale")).toBe("en");
  });
});
