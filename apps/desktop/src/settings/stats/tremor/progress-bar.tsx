// Tremor ProgressBar v0.0.3, Apache-2.0. See LICENSE.
// Source: tremorlabs/tremor@ca4d588f47820ff3d514d37fa4ee08a4222dec11.
// Adapted to Anarlog's theme tokens and reduced-motion preference.
import type { HTMLAttributes } from "react";

import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn } from "@anlg/utils";

export function ProgressBar({
  value = 0,
  max = 100,
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & { value?: number; max?: number }) {
  const trackRef = useSquircleRef<HTMLDivElement>();
  const fillRef = useSquircleRef<HTMLDivElement>();
  const safeMax = Math.max(0, max);
  const safeValue = Math.min(safeMax, Math.max(value, 0));
  return (
    <div
      className={cn(["flex w-full items-center", className])}
      role="progressbar"
      aria-valuenow={safeValue}
      aria-valuemin={0}
      aria-valuemax={safeMax}
      data-tremor-component="progress-bar"
      {...props}
    >
      <div
        ref={trackRef}
        className="bg-muted relative flex h-2 w-full items-center overflow-hidden rounded-full"
      >
        <div
          ref={fillRef}
          className="bg-foreground/80 h-full rounded-full transition-[width] duration-300 ease-in-out motion-reduce:transition-none"
          style={{ width: `${safeMax ? (safeValue / safeMax) * 100 : 0}%` }}
        />
      </div>
    </div>
  );
}
