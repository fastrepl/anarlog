import { describe, expect, test } from "vitest";

import {
  getLanguageDisplayName,
  getLanguageOptions,
  normalizeLanguageCodes,
} from "./language";

describe("language helpers", () => {
  test("shows base language names for locale variants", () => {
    expect(getLanguageDisplayName("ko-US")).toBe("Korean");
    expect(getLanguageDisplayName("en-US")).toBe("English");
  });

  test("normalizes and dedupes language selections by base language", () => {
    expect(
      normalizeLanguageCodes(["en-US", "en", "es-419", "es", "ko-US"]),
    ).toEqual(["en", "es", "ko"]);
  });

  test("builds one option per language while keeping variant aliases searchable", () => {
    const options = getLanguageOptions([
      "en",
      "en-US",
      "es",
      "es-419",
      "ko-KR",
    ]);

    expect(
      options.map(({ value, label }) => ({
        value,
        label,
      })),
    ).toEqual([
      { value: "en", label: "English" },
      { value: "es", label: "Spanish" },
      { value: "ko", label: "Korean" },
    ]);

    expect(
      options
        .find((option) => option.value === "es")
        ?.searchTerms.some((term) => term.includes("Latin")),
    ).toBe(true);
  });
});
