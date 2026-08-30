import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { panelSquircle } from "@anlg/ui/lib/squircle";
import { cn } from "@anlg/utils";

export const appFloatingContentClassName =
  "bg-app-floating-chrome text-popover-foreground border-app-floating-border overflow-hidden rounded-[22px] border p-0.5 shadow-lg";

export type FloatingContentVariant = "default" | "app";

export function AppFloatingPanel({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  const squircleRef = useSquircleRef<HTMLDivElement>(undefined, panelSquircle);
  return (
    <div
      ref={squircleRef}
      className={cn([
        "bg-app-floating-panel text-popover-foreground border-app-floating-border rounded-[18px] border",
        className,
      ])}
      {...props}
    />
  );
}
