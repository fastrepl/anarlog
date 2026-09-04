import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { squircleFocusVisibleClassName } from "@anlg/ui/lib/squircle";
import { cn } from "@anlg/utils";

const buttonVariants = cva(
  cn([
    squircleFocusVisibleClassName,
    "inline-flex cursor-pointer items-center justify-center gap-2 rounded-full text-sm font-medium whitespace-nowrap transition-all disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  ]),
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground hover:bg-primary/90 shadow-xs",
        destructive:
          "bg-destructive text-destructive-foreground hover:bg-destructive/90 shadow-xs",
        outline:
          "border-input bg-background hover:bg-accent hover:text-accent-foreground border shadow-xs",
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-secondary/80 shadow-xs",
        ghost: "hover:bg-accent hover:text-accent-foreground",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-7 px-2 text-xs",
        lg: "h-10 px-8",
        icon: "size-7",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

const DESTRUCTIVE_HOLD_ANIMATION = "destructive-button-hold";

export interface ButtonProps
  extends
    React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  smoothCorners?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      className,
      variant,
      size,
      asChild = false,
      smoothCorners = true,
      onBlur,
      disabled,
      onAnimationEnd,
      onAnimationEndCapture,
      onClick,
      onKeyDown,
      onKeyUp,
      onPointerCancel,
      onPointerDown,
      onPointerLeave,
      onPointerUp,
      ...props
    },
    ref,
  ) => {
    const [isHolding, setIsHolding] = React.useState(false);
    const allowClickRef = React.useRef(false);
    const holdInputRef = React.useRef<"keyboard" | "pointer" | null>(null);
    const Comp = asChild ? Slot : "button";
    const squircleRef = useSquircleRef(ref);
    const requiresHold = variant === "destructive" && !disabled;

    const cancelHold = () => {
      holdInputRef.current = null;
      setIsHolding(false);
    };

    return (
      <Comp
        className={cn([
          buttonVariants({ variant, size, className }),
          variant === "destructive" && [
            "relative overflow-hidden",
            "before:bg-destructive-foreground/80 before:pointer-events-none before:absolute before:inset-x-0 before:bottom-0 before:h-0.5 before:origin-left before:content-['']",
            isHolding
              ? "before:animate-[destructive-button-hold_1.2s_linear_forwards] before:opacity-100"
              : "before:opacity-0",
          ],
        ])}
        data-hold-state={
          variant === "destructive"
            ? isHolding
              ? "holding"
              : "idle"
            : undefined
        }
        onBlur={(event) => {
          onBlur?.(event);
          if (holdInputRef.current === "keyboard") cancelHold();
        }}
        disabled={disabled}
        onAnimationEnd={onAnimationEnd}
        onAnimationEndCapture={(event) => {
          onAnimationEndCapture?.(event);
          if (event.animationName !== DESTRUCTIVE_HOLD_ANIMATION) return;

          onAnimationEnd?.(event);
          event.stopPropagation();
          if (!requiresHold || !isHolding) return;

          cancelHold();
          allowClickRef.current = true;
          try {
            event.currentTarget.click();
          } finally {
            allowClickRef.current = false;
          }
        }}
        onClick={(event) => {
          if (!requiresHold || allowClickRef.current) {
            onClick?.(event);
            return;
          }

          event.preventDefault();
          event.stopPropagation();
        }}
        onKeyDown={(event) => {
          onKeyDown?.(event);
          if (
            event.defaultPrevented ||
            !requiresHold ||
            event.repeat ||
            (event.key !== "Enter" && event.key !== " ")
          ) {
            return;
          }

          event.preventDefault();
          holdInputRef.current = "keyboard";
          setIsHolding(true);
        }}
        onKeyUp={(event) => {
          onKeyUp?.(event);
          if (
            holdInputRef.current === "keyboard" &&
            (event.key === "Enter" || event.key === " ")
          ) {
            event.preventDefault();
            cancelHold();
          }
        }}
        onPointerCancel={(event) => {
          onPointerCancel?.(event);
          if (holdInputRef.current === "pointer") cancelHold();
        }}
        onPointerDown={(event) => {
          onPointerDown?.(event);
          if (
            event.defaultPrevented ||
            !requiresHold ||
            !event.isPrimary ||
            event.button !== 0
          ) {
            return;
          }

          holdInputRef.current = "pointer";
          setIsHolding(true);
        }}
        onPointerLeave={(event) => {
          onPointerLeave?.(event);
          if (holdInputRef.current === "pointer") cancelHold();
        }}
        onPointerUp={(event) => {
          onPointerUp?.(event);
          if (holdInputRef.current === "pointer") cancelHold();
        }}
        {...props}
        ref={smoothCorners ? squircleRef : ref}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
