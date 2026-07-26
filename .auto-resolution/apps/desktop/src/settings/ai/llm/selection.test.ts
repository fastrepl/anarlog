import { describe, expect, test } from "vitest";

import {
  getDefaultLlmSelection,
  getPreferredProviderModel,
  isSameModelSelection,
  shouldShowMissingModelWarning,
} from "./selection";

describe("isSameModelSelection", () => {
  test("matches only when both provider and model match", () => {
    expect(isSameModelSelection("openai", "gpt-5.5", "openai", "gpt-5.5")).toBe(
      true,
    );
    expect(isSameModelSelection("openai", "gpt-5.5", "openai", "gpt-5")).toBe(
      false,
    );
    expect(
      isSameModelSelection("openai", "gpt-5.5", "anthropic", "gpt-5.5"),
    ).toBe(false);
  });
});

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

describe("shouldShowMissingModelWarning", () => {
  test("stays quiet while a provider selection is resolving", () => {
    expect(
      shouldShowMissingModelWarning({
        isConfigured: false,
        isResolvingSelection: true,
        providerSettingsReady: true,
        settingsReady: true,
      }),
    ).toBe(false);
  });

  test("stays quiet until application settings are loaded", () => {
    expect(
      shouldShowMissingModelWarning({
        isConfigured: false,
        isResolvingSelection: false,
        providerSettingsReady: true,
        settingsReady: false,
      }),
    ).toBe(false);
  });

  test("warns when the settled selection has no model", () => {
    expect(
      shouldShowMissingModelWarning({
        isConfigured: false,
        isResolvingSelection: false,
        providerSettingsReady: true,
        settingsReady: true,
      }),
    ).toBe(true);
  });
});
