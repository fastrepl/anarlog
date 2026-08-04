import { Trans } from "@lingui/react/macro";
import type { ReactNode } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

export function DestructiveConfirmationDialog({
  open,
  onOpenChange,
  title,
  description,
  icon,
  confirmLabel,
  pendingLabel,
  isPending = false,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: ReactNode;
  description: ReactNode;
  icon: ReactNode;
  confirmLabel: ReactNode;
  pendingLabel?: ReactNode;
  isPending?: boolean;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
        <DialogHeader className="items-center gap-2 px-5 pt-7 text-center sm:text-center">
          <div className="relative -my-2 flex size-[92px] items-center justify-center overflow-visible">
            <div
              aria-hidden="true"
              className="bg-destructive/20 absolute size-14 rounded-[18px] blur-md"
            />
            <div className="border-destructive/20 bg-destructive/10 text-destructive relative flex size-14 items-center justify-center rounded-[18px] border shadow-[0_1px_0_rgba(255,255,255,0.65),0_10px_24px_-10px_rgba(0,0,0,0.48)] [&_svg]:size-7">
              {icon}
            </div>
          </div>
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            {title}
          </DialogTitle>
          <DialogDescription className="text-foreground max-w-[260px] text-center text-[13px] leading-[1.36]">
            {description}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 px-4 pt-4 pb-4 sm:grid-cols-2 sm:justify-normal">
          <Button
            variant="ghost"
            className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-4 text-xs font-medium shadow-none"
            disabled={isPending}
            onClick={() => onOpenChange(false)}
          >
            <Trans>Cancel</Trans>
          </Button>
          <Button
            variant="destructive"
            className="h-8 rounded-full px-4 text-xs font-medium shadow-sm"
            disabled={isPending}
            onClick={onConfirm}
          >
            {isPending ? (pendingLabel ?? confirmLabel) : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
