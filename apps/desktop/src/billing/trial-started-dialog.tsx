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

interface TrialStartedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trialDaysRemaining: number | null;
}

export function TrialStartedDialog({
  open,
  onOpenChange,
  trialDaysRemaining,
}: TrialStartedDialogProps) {
  const days = trialDaysRemaining ?? 14;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] border-white/45 bg-neutral-200/95 p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
        <DialogHeader className="items-center gap-2 px-5 pt-7 text-center sm:text-center">
          <TrialDialogIcon state="started" />
          <DialogTitle className="text-[13px] leading-5 font-semibold tracking-normal text-neutral-900">
            Your Pro trial just started
          </DialogTitle>
          <DialogDescription className="w-full text-center text-[13px] leading-[1.36] text-neutral-800">
            Your {days}-day Pro trial starts now. No payment needed.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="px-4 pt-4 pb-4 sm:justify-center">
          <Button
            className="h-8 w-full rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-xs font-medium text-white shadow-sm hover:from-stone-500 hover:to-stone-500 hover:text-white"
            onClick={() => onOpenChange(false)}
          >
            Got it
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
