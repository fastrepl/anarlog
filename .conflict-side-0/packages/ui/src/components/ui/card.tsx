import { forwardRef } from "react";

import { cn } from "@hypr/utils";

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: "default" | "outline-solid" | "ghost";
}

export const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, variant = "default", ...props }, ref) => (
    <div
      ref={ref}
      className={cn([
        "rounded-lg",
        variant === "default" && "bg-background border shadow-xs",
        variant === "outline-solid" && "border",
        variant === "ghost" && "border-none shadow-none",
        className,
      ])}
      {...props}
    />
  ),
);
Card.displayName = "Card";

interface CardHeaderProps extends React.HTMLAttributes<HTMLDivElement> {
  spacing?: "default" | "compact" | "loose";
}

export const CardHeader = forwardRef<HTMLDivElement, CardHeaderProps>(
  ({ className, spacing = "default", ...props }, ref) => (
    <div
      ref={ref}
      className={cn([
        "flex flex-col",
        spacing === "default" && "gap-1.5 p-6",
        spacing === "compact" && "gap-1 p-4",
        spacing === "loose" && "gap-2 p-8",
        className,
      ])}
      {...props}
    />
  ),
);
CardHeader.displayName = "CardHeader";

interface CardTitleProps extends React.HTMLAttributes<HTMLHeadingElement> {
  as?: "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
}

export const CardTitle = forwardRef<HTMLHeadingElement, CardTitleProps>(
  ({ className, as: Component = "h3", ...props }, ref) => (
    <Component
      ref={ref}
      className={cn(["leading-none font-semibold tracking-tight", className])}
      {...props}
    />
  ),
);
CardTitle.displayName = "CardTitle";

interface CardDescriptionProps extends React.HTMLAttributes<HTMLParagraphElement> {}

export const CardDescription = forwardRef<
  HTMLParagraphElement,
  CardDescriptionProps
>(({ className, ...props }, ref) => (
  <p
    ref={ref}
    className={cn(["text-sm text-neutral-500", className])}
    {...props}
  />
));
CardDescription.displayName = "CardDescription";

interface CardContentProps extends React.HTMLAttributes<HTMLDivElement> {
  spacing?: "default" | "compact" | "loose";
}

export const CardContent = forwardRef<HTMLDivElement, CardContentProps>(
  ({ className, spacing = "default", ...props }, ref) => (
    <div
      ref={ref}
      className={cn([
        spacing === "default" && "p-6 pt-0",
        spacing === "compact" && "p-4 pt-0",
        spacing === "loose" && "p-8 pt-0",
        className,
      ])}
      {...props}
    />
  ),
);
CardContent.displayName = "CardContent";

interface CardFooterProps extends React.HTMLAttributes<HTMLDivElement> {
  spacing?: "default" | "compact" | "loose";
  align?: "start" | "center" | "end" | "between" | "around" | "evenly";
}

export const CardFooter = forwardRef<HTMLDivElement, CardFooterProps>(
  ({ className, spacing = "default", align = "between", ...props }, ref) => (
    <div
      ref={ref}
      className={cn([
        "flex",
        spacing === "default" && "p-6 pt-0",
        spacing === "compact" && "p-4 pt-0",
        spacing === "loose" && "p-8 pt-0",
        align === "start" && "justify-start",
        align === "center" && "justify-center",
        align === "end" && "justify-end",
        align === "between" && "justify-between",
        align === "around" && "justify-around",
        align === "evenly" && "justify-evenly",
        className,
      ])}
      {...props}
    />
  ),
);
CardFooter.displayName = "CardFooter";
