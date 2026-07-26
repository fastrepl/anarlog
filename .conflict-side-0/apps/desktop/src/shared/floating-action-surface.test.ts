import { describe, expect, it } from "vitest";

import { floatingActionSurfaceClassName } from "./floating-action-surface";

describe("floatingActionSurfaceClassName", () => {
  it("uses an inverted surface against light and dark app chrome", () => {
    expect(floatingActionSurfaceClassName).toContain("bg-foreground");
    expect(floatingActionSurfaceClassName).toContain("text-background");
    expect(floatingActionSurfaceClassName).toContain("dark:bg-white/92");
    expect(floatingActionSurfaceClassName).toContain("dark:text-primary");
  });
});
