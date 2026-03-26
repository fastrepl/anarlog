import { describe, expect, test } from "vitest";

import { getPreferredProviderModel } from "./selection";

describe("getPreferredProviderModel", () => {
  test("returns the remembered model when it is still available", () => {
    expect(
      getPreferredProviderModel("claude-3-7-sonnet", [
        "claude-3-5-sonnet",
        "claude-3-7-sonnet",
      ]),
    ).toBe("claude-3-7-sonnet");
  });

  test("falls back to the first available model when none is remembered", () => {
    expect(getPreferredProviderModel(undefined, ["gpt-4.1", "gpt-4o"])).toBe(
      "gpt-4.1",
    );
  });

  test("falls back to the first available model when the remembered model is gone", () => {
    expect(
      getPreferredProviderModel("claude-3-opus", [
        "claude-3-5-sonnet",
        "claude-3-7-sonnet",
      ]),
    ).toBe("claude-3-5-sonnet");
  });

  test("clears the selection when a provider has no selectable models", () => {
    expect(getPreferredProviderModel("gpt-4.1", [])).toBe("");
  });

  test("keeps the remembered value when the provider does not expose a static list", () => {
    expect(
      getPreferredProviderModel("my-custom-model", [], {
        allowSavedModelWithoutChoices: true,
      }),
    ).toBe("my-custom-model");
  });
});
