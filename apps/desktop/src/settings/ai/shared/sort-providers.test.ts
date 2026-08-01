import { describe, expect, test } from "vitest";

import { sortProviders } from "./sort-providers";

describe("sortProviders", () => {
  test("keeps Anarlog first and Custom last", () => {
    const sorted = sortProviders(
      [
        { id: "custom", displayName: "Custom" },
        { id: "fireworks", displayName: "Fireworks", disabled: true },
        { id: "openai", displayName: "OpenAI" },
        { id: "anarlog", displayName: "Anarlog" },
      ],
      ["fireworks", "openai"],
    );

    expect(sorted.map((provider) => provider.id)).toEqual([
      "anarlog",
      "openai",
      "fireworks",
      "custom",
    ]);
  });

  test("uses the preferred order before the alphabetical fallback", () => {
    const sorted = sortProviders(
      [
        { id: "mistral", displayName: "Mistral" },
        { id: "unknown-b", displayName: "Beta" },
        { id: "openai", displayName: "OpenAI" },
        { id: "unknown-a", displayName: "Alpha" },
        { id: "anthropic", displayName: "Anthropic" },
      ],
      ["openai", "anthropic", "mistral"],
    );

    expect(sorted.map((provider) => provider.id)).toEqual([
      "openai",
      "anthropic",
      "mistral",
      "unknown-a",
      "unknown-b",
    ]);
  });
});
