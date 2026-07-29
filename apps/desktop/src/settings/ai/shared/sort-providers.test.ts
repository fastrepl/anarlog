import { describe, expect, test } from "vitest";

import { sortProviders } from "./sort-providers";

describe("sortProviders", () => {
  test("keeps Anarlog first and Custom last", () => {
    const sorted = sortProviders([
      { id: "custom", displayName: "Custom" },
      { id: "fireworks", displayName: "Fireworks", disabled: true },
      { id: "openai", displayName: "OpenAI" },
      { id: "anarlog", displayName: "Anarlog" },
    ]);

    expect(sorted.map((provider) => provider.id)).toEqual([
      "anarlog",
      "openai",
      "fireworks",
      "custom",
    ]);
  });
});
