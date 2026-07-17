import { describe, expect, test } from "vitest";

import { getDefaultLlmSelection, getPreferredProviderModel } from "./selection";

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

describe("getDefaultLlmSelection", () => {
  test("keeps the active provider and repairs its missing model", async () => {
    const selection = await getDefaultLlmSelection(
      ["openai", "anthropic"],
      "openai",
      undefined,
      async (provider) =>
        provider === "openai" ? ["gpt-5.5"] : ["claude-sonnet-4-5"],
    );

    expect(selection).toEqual({ provider: "openai", model: "gpt-5.5" });
  });

  test("skips providers whose models cannot be loaded", async () => {
    const selection = await getDefaultLlmSelection(
      ["openai", "anthropic"],
      undefined,
      undefined,
      async (provider) => {
        if (provider === "openai") {
          throw new Error("invalid key");
        }

        return ["claude-sonnet-4-5"];
      },
    );

    expect(selection).toEqual({
      provider: "anthropic",
      model: "claude-sonnet-4-5",
    });
  });

  test("returns no selection when no configured provider has models", async () => {
    const selection = await getDefaultLlmSelection(
      ["openai"],
      undefined,
      undefined,
      async () => [],
    );

    expect(selection).toBeNull();
  });
});
