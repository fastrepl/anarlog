import { SmoothCorners, type SmoothCornersProps } from "@lisse/react";

import { controlSquircle } from "@anlg/ui/lib/squircle";

export function Squircle({
  corners = controlSquircle,
  autoEffects = true,
  shadowStrategy = "box-shadow",
  ...props
}: SmoothCornersProps) {
  return (
    <SmoothCorners
      corners={corners}
      autoEffects={autoEffects}
      shadowStrategy={shadowStrategy}
      {...props}
    />
  );
}
