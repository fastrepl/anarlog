import React from "react";

import { cn } from "@hypr/utils";

interface SpinnerProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: number;
  color?: string;
}

const Spinner = React.forwardRef<HTMLDivElement, SpinnerProps>(
  ({ size = 16, color = "currentColor", className, ...props }, ref) => {
    const segments = Array.from({ length: 12 });

    return (
      <div
        ref={ref}
        className={cn(["relative inline-block", className])}
        style={{
          width: size,
          height: size,
          color: color,
        }}
        {...props}
      >
        {segments.map((_, i) => {
          const rotation = i * 30;
          const animationDelay = `${i * (1 / 12)}s`;

          return (
            <div
              key={i}
              className="absolute top-0 left-0 h-full w-full"
              style={{
                transform: `rotate(${rotation}deg)`,
              }}
            >
              <div
                className="animate-ios-opacity-spin absolute rounded-full bg-current"
                style={{
                  left: "50%",
                  top: "0",
                  width: `${size * 0.1}px`,
                  height: `${size * 0.26}px`,
                  transform: "translateX(-50%)",
                  animationDelay,
                }}
              />
            </div>
          );
        })}
      </div>
    );
  },
);

Spinner.displayName = "Spinner";

export { Spinner };
