import type { ComponentProps } from "react";

import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { panelSquircle } from "@anlg/ui/lib/squircle";
import { cn } from "@anlg/utils";

// Nested radii stay concentric: outer = inner + padding.
export const APP_FLOATING_CHROME_RADIUS_PX = 22;
export const APP_FLOATING_CHROME_PADDING_PX = 2;
export const APP_FLOATING_PANEL_RADIUS_PX =
  APP_FLOATING_CHROME_RADIUS_PX - APP_FLOATING_CHROME_PADDING_PX;
export const APP_FLOATING_PANEL_PADDING_PX = 6;
export const APP_FLOATING_ITEM_RADIUS_PX =
  APP_FLOATING_PANEL_RADIUS_PX - APP_FLOATING_PANEL_PADDING_PX;
export const FLOATING_MENU_RADIUS_PX = 18;
export const FLOATING_MENU_PADDING_PX = 4;
export const FLOATING_MENU_ITEM_RADIUS_PX =
  FLOATING_MENU_RADIUS_PX - FLOATING_MENU_PADDING_PX;

export const appFloatingContentClassName =
  "bg-app-floating-chrome text-popover-foreground border-app-floating-border overflow-hidden rounded-[22px] border p-0.5 shadow-lg";

export const appFloatingMenuPanelClassName = "overflow-hidden p-1.5";
export const appFloatingItemClassName = "rounded-[14px]";
export const floatingMenuItemClassName = "rounded-[14px]";

export type FloatingContentVariant = "default" | "app";

export function AppFloatingPanel({
  className,
  ref,
  ...props
}: ComponentProps<"div">) {
  const squircleRef = useSquircleRef<HTMLDivElement>(ref, panelSquircle);
  return (
    <div
      className={cn([
        "bg-app-floating-panel text-popover-foreground border-app-floating-border rounded-[20px] border",
        className,
      ])}
      {...props}
      ref={squircleRef}
    />
  );
}
