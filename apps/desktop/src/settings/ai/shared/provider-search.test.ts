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
  test.each([
    ["MOON", [0]],
    ["alibaba_cloud", [1]],
    ["  ", [0, 1, 2, 3]],
    ["whisper", [3]],
    [".bin", [3]],
  ])("filters providers for %s", (query, expectedIndexes) => {
    expect(filterProviders(providers, query)).toEqual(
      expectedIndexes.map((index) => providers[index]),
    );
  });
});
