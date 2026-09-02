import { describe, expect, it } from "vitest";

import {
  boundedMinWidthPx,
  getMainContentMinWidth,
  SETTINGS_SURFACE_MIN_WIDTH_PX,
} from "./layout-widths";

describe("layout-widths", () => {
  it("keeps a settings surface preference without overflowing its parent", () => {
    expect(getMainContentMinWidth({ type: "settings" })).toBe(
      SETTINGS_SURFACE_MIN_WIDTH_PX,
    );
    expect(boundedMinWidthPx(SETTINGS_SURFACE_MIN_WIDTH_PX)).toBe(
      "min(700px, 100%)",
    );
  });
});
