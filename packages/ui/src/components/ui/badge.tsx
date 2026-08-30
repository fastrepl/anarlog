import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn } from "@anlg/utils";

const badgeVariants = cva(
  "focus:outline-ring inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-2 focus:outline-offset-2",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground hover:bg-primary/80 border-transparent",
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-secondary/80 border-transparent",
        destructive:
          "bg-destructive text-destructive-foreground hover:bg-destructive/80 border-transparent",
        outline: "text-foreground",
        success:
          "border-transparent bg-green-500 text-white hover:bg-green-600",
      },
      size: {
        default: "px-2.5 py-0.5 text-xs",
        sm: "px-2 py-0.5 text-xs",
        lg: "px-3 py-1 text-sm",
      },
      disabled: {
        true: "pointer-events-none cursor-not-allowed opacity-50",
        false: "",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
      disabled: false,
    },
  },
);

export interface BadgeProps
  extends
    React.HTMLAttributes<HTMLDivElement>,
    React.RefAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {
  label?: string;
}

function Badge({
  className,
  variant,
  size,
  disabled,
  label,
  children,
  ref,
  ...props
}: BadgeProps) {
  const squircleRef = useSquircleRef<HTMLDivElement>(ref);
  return (
    <div
      className={cn([badgeVariants({ variant, size, disabled }), className])}
      aria-label={label}
      role="status"
      {...props}
      ref={squircleRef}
    >
      {children}
    </div>
  );
}

export { Badge, badgeVariants };
