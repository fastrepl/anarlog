import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  storedValues: {} as Record<string, string>,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: () =>
    Promise.resolve({
      values: { ...mocks.storedValues },
      hasValues: new Set(Object.keys(mocks.storedValues)),
    }),
  setSettingValues: async (values: Record<string, string>) => {
    await new Promise((resolve) => setTimeout(resolve, 5));
    Object.assign(mocks.storedValues, values);
  },
}));

import { buildTrayLanguageOptions, selectTrayLanguage } from "./tray-languages";

function storedLanguages() {
  return {
    ai_language: mocks.storedValues.ai_language,
    spoken_languages: JSON.parse(mocks.storedValues.spoken_languages),
  };
}

describe("buildTrayLanguageOptions", () => {
  test("lists the main language first and selected", () => {
    expect(buildTrayLanguageOptions("es", ["en"])).toEqual([
      { code: "es", label: "Spanish", selected: true },
      { code: "en", label: "English", selected: false },
    ]);
  });
});

describe("selectTrayLanguage", () => {
  beforeEach(() => {
    mocks.storedValues = {};
  });

  test("switching away and back keeps a regional main language", async () => {
    mocks.storedValues = {
      ai_language: "en-US",
      spoken_languages: JSON.stringify(["es"]),
    };

    await selectTrayLanguage("es");
    expect(storedLanguages()).toEqual({
      ai_language: "es",
      spoken_languages: ["en-US"],
    });

    await selectTrayLanguage("en-US");
    expect(storedLanguages()).toEqual({
      ai_language: "en-US",
      spoken_languages: ["es"],
    });
  });

  test("rapid switches persist the last choice", async () => {
    mocks.storedValues = {
      ai_language: "es",
      spoken_languages: JSON.stringify(["en"]),
    };

    await Promise.all([selectTrayLanguage("en"), selectTrayLanguage("es")]);

    expect(storedLanguages()).toEqual({
      ai_language: "es",
      spoken_languages: ["en"],
    });
  });
});
