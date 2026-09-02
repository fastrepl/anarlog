import { APPLE_SMOOTHING, type SmoothCornerOptions } from "@lisse/core";

import { DesignRadius } from "@anlg/design-system";

export const controlSquircle = {
  radius: DesignRadius.lg,
  smoothing: APPLE_SMOOTHING,
} satisfies SmoothCornerOptions;

export const panelSquircle = {
  radius: DesignRadius.panel,
  smoothing: APPLE_SMOOTHING,
} satisfies SmoothCornerOptions;

export const squircleFocusVisibleClassName =
  "focus-visible:outline-ring focus-visible:outline-2 focus-visible:outline-offset-2";
