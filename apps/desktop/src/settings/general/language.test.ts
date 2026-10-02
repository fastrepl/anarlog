import { describe, expect, test } from "vitest";

import {
  CORE_TRANSCRIPTION_LANGUAGE_CODES,
  getAdditionalSpokenLanguages,
  getBaseLanguageCode,
  parseLocale,
} from "./language";

describe("getAdditionalSpokenLanguages", () => {
  test("removes the main language from stored spoken languages", () => {
    expect(getAdditionalSpokenLanguages("en", ["en", "ko"])).toEqual(["ko"]);
  });

  test("matches regional variants by base language", () => {
    expect(getAdditionalSpokenLanguages("en-US", ["en", "ko-KR"])).toEqual([
      "ko",
    ]);
  });

  test("deduplicates additional languages", () => {
    expect(getAdditionalSpokenLanguages("en", ["ko", "ko-KR", "ja"])).toEqual([
      "ko",
      "ja",
    ]);
  });

  test("ignores malformed stored spoken languages", () => {
    expect(getAdditionalSpokenLanguages("en", ["not a locale", "ko"])).toEqual([
      "ko",
    ]);
  });

  test.each([
    ["tl", "tl"],
    ["fil-PH", "tl"],
    ["bh-IN", "bh"],
    ["bho-IN", "bh"],
    ["tw-GH", "tw"],
    ["ak-GH", "ak"],
  ])("normalizes transcription language alias %s to %s", (input, expected) => {
    expect(getBaseLanguageCode(input)).toBe(expected);
  });

  test("normalizes persisted Filipino aliases", () => {
    expect(getAdditionalSpokenLanguages("en", ["fil"])).toEqual(["tl"]);
  });

  test("uses a valid fallback while the main language is loading", () => {
    expect(parseLocale("")).toEqual({ language: "en" });
  });
});

describe("CORE_TRANSCRIPTION_LANGUAGE_CODES", () => {
  test("preserves every supported ISO 639-1 code", () => {
    for (const code of CORE_TRANSCRIPTION_LANGUAGE_CODES) {
      expect(getBaseLanguageCode(code)).toBe(code);
    }
  });
});
