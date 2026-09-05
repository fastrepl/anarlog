import {
  useSmoothCorners,
  type EffectsConfig,
  type SmoothCornerOptions,
} from "@lisse/react";
import { useCallback, useRef, type Ref } from "react";

import { controlSquircle } from "@anlg/ui/lib/squircle";

export function useSquircleRef<T extends HTMLElement>(
  forwardedRef?: Ref<T>,
  corners: SmoothCornerOptions = controlSquircle,
  effects?: EffectsConfig,
) {
  const localRef = useRef<T | null>(null);
  const fallbackRadius =
    "radius" in corners && typeof corners.radius === "number"
      ? `${corners.radius}px`
      : `${controlSquircle.radius}px`;

  useSmoothCorners(localRef, corners, {
    autoEffects: true,
    effects,
    // CSS radius intersects clip-path and squares the curve; Lisse clears this
    // after the clip-path lands and restores it on teardown.
    fallbackBorderRadius: fallbackRadius,
    // Omit skipShadowHandle so extracted box-shadows (shadow-xs) paint on
    // the parent overlay instead of being clipped on the host.
  });

  return useCallback(
    (node: T | null) => {
      localRef.current = node;
      if (typeof forwardedRef === "function") {
        forwardedRef(node);
      } else if (forwardedRef) {
        forwardedRef.current = node;
      }
    },
    [forwardedRef],
  );
}
