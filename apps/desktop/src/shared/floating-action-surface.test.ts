import * as stylex from "@stylexjs/stylex";
import { describe, expect, it } from "vitest";

import { floatingActionSurfaceStyles } from "./floating-action-surface";

describe("floatingActionSurfaceStyles", () => {
  it("provides the inverted surface through StyleX", () => {
    expect(
      stylex.props(floatingActionSurfaceStyles.root).className,
    ).toBeTruthy();
  });
});
