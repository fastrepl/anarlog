import { describe, expect, test } from "vitest";

import { resolveDisplayLocale } from "./locales";

describe("resolveDisplayLocale", () => {
  test("uses exact supported locales", () => {
    expect(resolveDisplayLocale("ko")).toBe("ko");
  });

  test("uses base language for regional variants", () => {
    expect(resolveDisplayLocale("ja-JP")).toBe("ja");
  });

  test("falls back to English for unsupported languages", () => {
    expect(resolveDisplayLocale("pl")).toBe("en");
  });

  test("falls back to English for invalid values", () => {
    expect(resolveDisplayLocale("not a locale")).toBe("en");
  });
});
