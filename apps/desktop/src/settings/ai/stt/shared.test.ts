import { describe, expect, test } from "vitest";

import { displayModelLabel, displayModelTitle } from "./shared";

describe("STT model display labels", () => {
  test("keeps cloud model product-facing", () => {
    expect(displayModelLabel("cloud")).toBe("Pro (Cloud)");
    expect(displayModelTitle("cloud")).toBeUndefined();
  });

  test("collapses local model names to on-device labels", () => {
    expect(
      displayModelLabel(
        "soniqo-parakeet-streaming",
        "Soniqo Parakeet Streaming",
      ),
    ).toBe("On device");
    expect(
      displayModelTitle(
        "soniqo-parakeet-streaming",
        "Soniqo Parakeet Streaming",
      ),
    ).toBe("Soniqo Parakeet Streaming");
  });
});
