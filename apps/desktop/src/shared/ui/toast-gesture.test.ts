import { describe, expect, it } from "vitest";

import { appToastSwipeDismissDirection } from "@anlg/ui/lib/toast-gesture";

describe("appToastSwipeDismissDirection", () => {
  it("dismisses through the side of a deliberate swipe", () => {
    expect(appToastSwipeDismissDirection(-80, 0)).toBe(-1);
    expect(appToastSwipeDismissDirection(80, 0)).toBe(1);
  });

  it("dismisses a fast flick and restores an accidental drag", () => {
    expect(appToastSwipeDismissDirection(12, -700)).toBe(-1);
    expect(appToastSwipeDismissDirection(20, 100)).toBeUndefined();
  });
});
