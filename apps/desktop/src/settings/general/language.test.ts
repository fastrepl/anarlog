import { describe, expect, test } from "vitest";

import { getAdditionalSpokenLanguages } from "./language";

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
});
