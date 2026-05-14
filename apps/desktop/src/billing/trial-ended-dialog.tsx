import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";

import { TrialDialogIcon } from "./trial-dialog-icon";

interface TrialEndedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onUpgrade: () => void;
}

export function TrialEndedDialog({
  open,
  onOpenChange,
  onUpgrade,
}: TrialEndedDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] border-white/45 bg-neutral-200/95 p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
        <DialogHeader className="items-center gap-2 px-5 pt-7 text-center sm:text-center">
          <TrialDialogIcon state="ended" />
          <DialogTitle className="text-[13px] leading-5 font-semibold tracking-normal text-neutral-900">
            Your Pro trial has ended
          </DialogTitle>
          <DialogDescription className="max-w-[260px] text-center text-[13px] leading-[1.36] text-neutral-800">
            You're back on the free plan. Your notes and recordings are safe —
            you can still capture and edit them locally. Upgrade to Pro to keep
            cloud AI, longer recordings, and premium templates.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 px-4 pt-4 pb-4 sm:grid-cols-2 sm:justify-normal">
          <Button
            variant="ghost"
            className="h-8 rounded-full bg-neutral-300/80 px-4 text-xs font-medium text-neutral-800 shadow-none hover:bg-neutral-300 hover:text-neutral-900"
            onClick={() => onOpenChange(false)}
          >
            Maybe later
          </Button>
          <Button
            className="h-8 rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-xs font-medium text-white shadow-sm hover:from-stone-500 hover:to-stone-500 hover:text-white"
            onClick={() => {
              onUpgrade();
              onOpenChange(false);
            }}
          >
            Upgrade to Pro
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
