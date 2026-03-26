import { describe, expect, test } from "vitest";

import {
  getBaseLanguageOptions,
  getSpokenLanguageDisplayName,
  getSpokenLanguageOptions,
  normalizeBaseLanguageCodes,
  normalizeSelectedSpokenLanguages,
  normalizeSpokenLanguageCodes,
} from "./language";

describe("language helpers", () => {
  test("shows meaningful spoken-language labels for supported variants", () => {
    expect(getSpokenLanguageDisplayName("ko-US")).toBe("Korean");
    expect(getSpokenLanguageDisplayName("en-US")).toBe("English (US)");
    expect(getSpokenLanguageDisplayName("zh-HK")).toBe("Cantonese");
    expect(getSpokenLanguageDisplayName("zh-TW")).toBe("Mandarin");
  });

  test("keeps meaningful spoken variants while collapsing provider-only locale noise", () => {
    expect(
      normalizeSpokenLanguageCodes([
        "en-US",
        "en",
        "es-419",
        "es",
        "ko-US",
        "zh-HK",
        "zh-TW",
      ]),
    ).toEqual(["en-US", "es-419", "ko", "zh-HK", "zh-TW"]);
  });

  test("keeps main-language options at the base-language level", () => {
    expect(
      getBaseLanguageOptions(["en", "en-US", "en-GB", "ko-KR"]).map(
        ({ value, label }) => ({ value, label }),
      ),
    ).toEqual([
      { value: "en", label: "English" },
      { value: "ko", label: "Korean" },
    ]);

    expect(normalizeBaseLanguageCodes(["en-US", "en", "ko-KR"])).toEqual([
      "en",
      "ko",
    ]);
  });

  test("builds spoken-language options with curated variant labels and aliases", () => {
    const options = getSpokenLanguageOptions([
      "en",
      "en-US",
      "en-GB",
      "es",
      "es-419",
      "ko-KR",
      "zh-HK",
      "zh-TW",
    ]);

    expect(
      options.map(({ value, label }) => ({
        value,
        label,
      })),
    ).toEqual([
      { value: "en", label: "English" },
      { value: "en-US", label: "English (US)" },
      { value: "en-GB", label: "English (UK)" },
      { value: "es", label: "Spanish" },
      { value: "es-419", label: "Spanish (Latin America)" },
      { value: "ko-KR", label: "Korean" },
      { value: "zh-HK", label: "Cantonese" },
      { value: "zh-TW", label: "Mandarin" },
    ]);

    expect(
      options
        .find((option) => option.value === "es-419")
        ?.searchTerms.some((term) => term.includes("Latin")),
    ).toBe(true);
  });

  test("maps legacy stored spoken languages onto current spoken-language options", () => {
    expect(
      normalizeSelectedSpokenLanguages(
        ["ko-US", "en-US", "zh-Hant"],
        ["en", "en-US", "ko-KR", "zh-TW", "zh-HK"],
      ),
    ).toEqual(["ko-KR", "en-US", "zh-TW"]);
  });
});
