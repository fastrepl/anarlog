import { describe, expect, it } from "vitest";

import {
  boundedMinWidthPx,
  getMainContentMinWidth,
  SETTINGS_SURFACE_MIN_WIDTH_PX,
} from "./layout-widths";

describe("layout-widths", () => {
  it("does not force a 700px inner settings panel beside the sidebar", () => {
    expect(getMainContentMinWidth({ type: "settings" })).toBeUndefined();
  });

  it("caps a preferred min-width so it cannot exceed its parent", () => {
    expect(boundedMinWidthPx(SETTINGS_SURFACE_MIN_WIDTH_PX)).toBe(
      "min(700px, 100%)",
    );
  });
});
