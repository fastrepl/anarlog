import { cn } from "@hypr/utils";

export const appFloatingContentClassName =
  "bg-app-floating-chrome text-popover-foreground border-app-floating-border overflow-hidden rounded-[22px] border p-1 shadow-lg";

export type FloatingContentVariant = "default" | "app";

export function AppFloatingPanel({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn([
        "bg-app-floating-panel text-popover-foreground border-app-floating-border rounded-[18px] border",
        className,
      ])}
      {...props}
    />
  );
}
