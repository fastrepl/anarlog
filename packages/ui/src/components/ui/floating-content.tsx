import { cn } from "@hypr/utils";

export const appFloatingContentClassName =
  "bg-popover text-popover-foreground overflow-hidden rounded-2xl border border-border p-1 shadow-lg";

export type FloatingContentVariant = "default" | "app";

export function AppFloatingPanel({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn([
        "bg-popover text-popover-foreground border-border rounded-2xl border",
        className,
      ])}
      {...props}
    />
  );
}
