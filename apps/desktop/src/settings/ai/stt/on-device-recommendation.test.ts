import { describe, expect, test } from "vitest";

import { recommendOnDeviceModel } from "./on-device-recommendation";

const GIB = 1024 * 1024 * 1024;
const models = [
  { id: "small", recommendedMemoryBytes: 8 * GIB },
  { id: "large", recommendedMemoryBytes: 16 * GIB },
];

describe("recommendOnDeviceModel", () => {
  test("chooses the most capable model that fits the available memory", () => {
    expect(recommendOnDeviceModel(models, 8 * GIB)).toBe("small");
    expect(recommendOnDeviceModel(models, 16 * GIB)).toBe("large");
    expect(recommendOnDeviceModel(models, 32 * GIB)).toBe("large");
  });

  test("uses the lightest model when memory is unknown or below the minimum", () => {
    expect(recommendOnDeviceModel(models)).toBe("small");
    expect(recommendOnDeviceModel(models, 4 * GIB)).toBe("small");
  });

  test("keeps source order when models have the same memory requirement", () => {
    expect(
      recommendOnDeviceModel(
        [
          { id: "first", recommendedMemoryBytes: 8 * GIB },
          { id: "second", recommendedMemoryBytes: 8 * GIB },
        ],
        16 * GIB,
      ),
    ).toBe("first");
  });

  test("returns null when the provider has no available models", () => {
    expect(recommendOnDeviceModel([], 16 * GIB)).toBeNull();
  });
});
