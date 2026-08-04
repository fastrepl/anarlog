import { PRO_TRIAL_DAYS } from "@anlg/pricing";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

import { TrialDialogIcon } from "./trial-dialog-icon";

import { GlassDialogContent } from "~/shared/ui/glass-dialog";

interface TrialStartedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trialDaysRemaining: number | null;
  hasPaymentMethod: boolean;
}

export function TrialStartedDialog({
  open,
  onOpenChange,
  trialDaysRemaining,
  hasPaymentMethod,
}: TrialStartedDialogProps) {
  const days = trialDaysRemaining ?? PRO_TRIAL_DAYS;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader className="items-center gap-2 text-center sm:text-center">
          <TrialDialogIcon state="started" />
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            Your Pro trial just started
          </DialogTitle>
          <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
            {hasPaymentMethod
              ? `Your ${days}-day Pro trial starts now. Pro will continue automatically when it ends.`
              : `Your ${days}-day Pro trial starts now. Add a payment method before it ends to keep Pro.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="sm:justify-center">
          <Button
            className="bg-primary text-primary-foreground hover:bg-primary/90 h-8 w-full rounded-full px-4 text-xs font-medium shadow-sm dark:bg-white dark:text-black dark:hover:bg-white/90"
            onClick={() => onOpenChange(false)}
          >
            Got it
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}
