import { describe, expect, it } from "vitest";

import { withCharUtm } from "./url";

describe("withCharUtm", () => {
  it("appends UTMs to char links", () => {
    expect(
      withCharUtm("https://char.com/download", {
        source: "github",
        medium: "readme",
      }),
    ).toBe(
      "https://char.com/download?utm_source=github&utm_medium=readme&utm_campaign=organic",
    );
  });

  it("preserves fragments and normalizes the homepage URL", () => {
    expect(
      withCharUtm("https://char.com#pricing", {
        source: "website",
        medium: "blog",
      }),
    ).toBe(
      "https://char.com/?utm_source=website&utm_medium=blog&utm_campaign=organic#pricing",
    );
  });

  it("keeps existing query params", () => {
    expect(
      withCharUtm("https://char.com/download?spec=apple-silicon", {
        source: "app",
        medium: "settings",
      }),
    ).toBe(
      "https://char.com/download?spec=apple-silicon&utm_source=app&utm_medium=settings&utm_campaign=organic",
    );
  });
});
