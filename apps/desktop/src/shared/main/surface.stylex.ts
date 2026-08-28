import * as stylex from "@stylexjs/stylex";

import { radii } from "@anlg/design-system/tokens.stylex";

export const mainSurface = stylex.defineVars({
  borderBottomWidth: "1px",
  borderLeftWidth: "1px",
  borderRightWidth: "1px",
  borderTopWidth: "1px",
  radiusBottomLeft: radii.xl,
  radiusBottomRight: radii.xl,
  radiusTopLeft: radii.xl,
  radiusTopRight: radii.xl,
});
