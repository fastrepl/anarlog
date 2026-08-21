import { describe, expect, it } from "vitest";

import { isFloatingBarSupported } from "./support";

describe("isFloatingBarSupported", () => {
  it("is available on desktop platforms", () => {
    expect(isFloatingBarSupported("macos")).toBe(true);
    expect(isFloatingBarSupported("windows")).toBe(true);
    expect(isFloatingBarSupported("linux")).toBe(true);
  });
});
