import * as AvatarPrimitive from "@radix-ui/react-avatar";
import * as React from "react";

import { cn } from "@hypr/utils";

interface AvatarProps extends React.ComponentPropsWithoutRef<
  typeof AvatarPrimitive.Root
> {
  variant?: "rounded" | "circle";
}

const Avatar = React.forwardRef<
  React.ComponentRef<typeof AvatarPrimitive.Root>,
  AvatarProps
>(({ className, variant = "circle", ...props }, ref) => (
  <AvatarPrimitive.Root
    ref={ref}
    className={cn([
      "relative flex h-10 w-10 shrink-0 overflow-hidden",
      variant === "circle" ? "rounded-full" : "rounded-lg",
      className,
    ])}
    {...props}
  />
));
Avatar.displayName = AvatarPrimitive.Root.displayName;

const AvatarImage = React.forwardRef<
  React.ComponentRef<typeof AvatarPrimitive.Image>,
  React.ComponentPropsWithoutRef<typeof AvatarPrimitive.Image>
>(({ className, ...props }, ref) => (
  <AvatarPrimitive.Image
    ref={ref}
    className={cn(["aspect-square h-full w-full", className])}
    {...props}
  />
));
AvatarImage.displayName = AvatarPrimitive.Image.displayName;

interface AvatarFallbackProps extends React.ComponentPropsWithoutRef<
  typeof AvatarPrimitive.Fallback
> {
  variant?: "rounded" | "circle";
}

const AvatarFallback = React.forwardRef<
  React.ComponentRef<typeof AvatarPrimitive.Fallback>,
  AvatarFallbackProps
>(({ className, variant = "circle", ...props }, ref) => (
  <AvatarPrimitive.Fallback
    ref={ref}
    className={cn([
      "bg-muted flex h-full w-full items-center justify-center",
      variant === "circle" ? "rounded-full" : "rounded-lg",
      className,
    ])}
    {...props}
  />
));
AvatarFallback.displayName = AvatarPrimitive.Fallback.displayName;

export { Avatar, AvatarFallback, AvatarImage };
