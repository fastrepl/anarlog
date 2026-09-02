import { describe, expect, test } from "vitest";

import { filterProviders } from "./provider-search";

const providers = [
  { id: "moonshot", displayName: "Moonshot AI" },
  { id: "alibaba_cloud", displayName: "Alibaba Cloud Model Studio" },
  { id: "zai", displayName: "Z.AI" },
  {
    id: "local_file",
    displayName: "On-device file",
    description: "whisper.cpp .bin",
  },
];

describe("filterProviders", () => {
  test("matches provider names without case sensitivity", () => {
    expect(filterProviders(providers, "MOON")).toEqual([providers[0]]);
  });

  test("matches provider identifiers", () => {
    expect(filterProviders(providers, "alibaba_cloud")).toEqual([providers[1]]);
  });

  test("returns all providers for a blank query", () => {
    expect(filterProviders(providers, "  ")).toEqual(providers);
  });

  test("matches provider descriptions", () => {
    expect(filterProviders(providers, "whisper")).toEqual([providers[3]]);
    expect(filterProviders(providers, ".bin")).toEqual([providers[3]]);
  });
});
