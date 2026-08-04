import type { ComponentProps } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { DialogContent } from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

export function GlassDialogContent({
  className,
  ...props
}: ComponentProps<typeof DialogContent>) {
  return (
    <DialogContent
      overlayClassName="bg-black/40"
      className={cn([
        "w-[calc(100vw-48px)] max-w-[320px] gap-4 overflow-hidden rounded-[26px] p-5 sm:rounded-[26px]",
        "border-border/45 bg-card/60 backdrop-blur-2xl backdrop-saturate-150",
        "shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_24px_70px_rgba(0,0,0,0.32)]",
        "[&>button:last-child]:hidden",
        className,
      ])}
      {...props}
    />
  );
}

export function GlassDialogCancelButton({
  className,
  ...props
}: ComponentProps<typeof Button>) {
  return (
    <Button
      variant="ghost"
      className={cn([
        "border-border/70 bg-background/50 text-foreground h-8 rounded-full border px-4 text-xs font-medium shadow-[0_1px_2px_rgba(0,0,0,0.06)]",
        "hover:bg-background/80 hover:text-foreground",
        className,
      ])}
      {...props}
    />
  );
}
